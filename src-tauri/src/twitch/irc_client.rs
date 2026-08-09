//! Client de connexion IRC-over-WebSocket pour **une** chaîne Twitch.
//!
//! ## Pourquoi IRC WebSocket plutôt qu'EventSub ?
//!
//! Twitch expose deux façons de recevoir les messages de chat :
//!
//! 1. **IRC over WebSocket** (`wss://irc-ws.chat.twitch.tv:443`) : protocole
//!    historique, stable depuis des années, ne nécessite pas de gestion de
//!    souscriptions ni de renouvellement de session complexe. Fonctionne
//!    même en lecture anonyme (`justinfan12345`).
//! 2. **EventSub WebSocket** (`channel.chat.message`) : API plus moderne,
//!    payloads plus riches nativement (fragments d'emotes structurés), mais
//!    impose une gestion de session (`session_welcome`, `session_reconnect`,
//!    keepalive à intervalle variable) et une souscription Helix par type
//!    d'évènement et par chaîne — complexité supplémentaire pour un gain
//!    marginal ici (les tags IRC suffisent à extraire badges/couleur/rôle).
//!
//! **Décision : IRC WebSocket pour le MVP**, avec ce module isolé derrière
//! le trait implicite exposé par [`super::connection_manager`] afin de
//! pouvoir ajouter une implémentation EventSub en Phase 2+ sans toucher au
//! reste de l'application (voir cahier technique, section 4.2).

use crate::twitch::message::{is_ping, try_parse_privmsg, ChatMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

const TWITCH_IRC_WS_URL: &str = "wss://irc-ws.chat.twitch.tv:443";

/// Évènements remontés par une connexion de chaîne vers le
/// [`super::connection_manager::ConnectionManager`].
#[derive(Debug, Clone)]
pub enum ChannelEvent {
    Connected,
    Disconnected { reason: String },
    Message(ChatMessage),
    /// Notice serveur (ex: `msg_channel_suspended`) affichée en diagnostic.
    Notice(String),
}

pub struct IrcChannelClient {
    channel_login: String,
    oauth_token: String,
    bot_login: String,
}

impl IrcChannelClient {
    pub fn new(channel_login: String, oauth_token: String, bot_login: String) -> Self {
        Self {
            channel_login: channel_login.to_lowercase(),
            oauth_token,
            bot_login,
        }
    }

    /// Ouvre la connexion WebSocket, s'authentifie, rejoint le canal, puis
    /// relaie les évènements via `tx` jusqu'à déconnexion (volontaire ou non).
    /// Cette fonction ne retourne qu'en cas de fermeture de la connexion :
    /// c'est au [`ConnectionManager`](super::connection_manager::ConnectionManager)
    /// d'implémenter la logique de reconnexion (backoff exponentiel).
    pub async fn run(&self, tx: mpsc::UnboundedSender<ChannelEvent>) -> anyhow::Result<()> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(TWITCH_IRC_WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();

        // Capacités IRCv3 nécessaires pour recevoir les tags (badges, couleur, id...)
        write
            .send(WsMessage::Text(
                "CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership".into(),
            ))
            .await?;
        write
            .send(WsMessage::Text(format!("PASS oauth:{}", self.oauth_token)))
            .await?;
        write
            .send(WsMessage::Text(format!("NICK {}", self.bot_login)))
            .await?;
        write
            .send(WsMessage::Text(format!("JOIN #{}", self.channel_login)))
            .await?;

        tx.send(ChannelEvent::Connected).ok();

        while let Some(msg) = read.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    tx.send(ChannelEvent::Disconnected {
                        reason: format!("Erreur WebSocket : {e}"),
                    })
                    .ok();
                    return Err(e.into());
                }
            };

            let Some(text) = msg.to_text().ok() else {
                continue;
            };

            for line in text.split("\r\n").filter(|l| !l.is_empty()) {
                if is_ping(line) {
                    // Le serveur exige une réponse PONG rapide, sinon il coupe la connexion.
                    write.send(WsMessage::Text("PONG :tmi.twitch.tv".into())).await?;
                    continue;
                }

                if line.contains("NOTICE") {
                    tx.send(ChannelEvent::Notice(line.to_string())).ok();
                    // Certaines NOTICE indiquent un échec d'authentification définitif.
                    if line.contains("Login authentication failed")
                        || line.contains("Improperly formatted auth")
                    {
                        tx.send(ChannelEvent::Disconnected {
                            reason: "Authentification refusée par Twitch".into(),
                        })
                        .ok();
                        return Err(anyhow::anyhow!("auth refusée"));
                    }
                    continue;
                }

                if let Some(chat_message) = try_parse_privmsg(line, &self.channel_login) {
                    tx.send(ChannelEvent::Message(chat_message)).ok();
                }
            }
        }

        tx.send(ChannelEvent::Disconnected {
            reason: "Flux WebSocket clos par le serveur".into(),
        })
        .ok();
        Ok(())
    }
}
