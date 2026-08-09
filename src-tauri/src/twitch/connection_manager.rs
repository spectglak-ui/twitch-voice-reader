//! Orchestration des connexions à plusieurs chaînes Twitch simultanément.
//!
//! Modèle retenu : **un acteur asynchrone par chaîne** (une tâche dédiée
//! exécutant [`IrcChannelClient::run`] en boucle, spawnée via
//! `tauri::async_runtime::spawn` — voir la note ci-dessous). Chaque acteur :
//! - possède son propre état de reconnexion (backoff exponentiel indépendant),
//! - peut être arrêté individuellement sans affecter les autres chaînes,
//! - remonte ses évènements vers un canal `mpsc` central partagé.
//!
//! Alternative écartée : une seule connexion WebSocket multiplexant tous
//! les `JOIN` sur un unique socket. C'est possible avec IRC Twitch, mais
//! cela couple la durée de vie de toutes les chaînes à une seule connexion
//! (une déconnexion réseau affecte tout le monde, un rate-limit sur un JOIN
//! peut bloquer les autres) — le modèle par acteur isolé est plus robuste
//! et plus simple à raisonner, au prix d'un socket TCP de plus par chaîne
//! (négligeable pour l'usage visé : quelques chaînes en parallèle).
//!
//! ## `tauri::async_runtime::spawn` plutôt que `tokio::spawn`
//!
//! `connect()` peut être appelé depuis un contexte où le handle de runtime
//! Tokio n'est pas garanti "entré" sur le thread appelant (ex: tâche de
//! reconnexion automatique lancée depuis `Tauri::setup`, qui est un
//! callback synchrone). `tauri::async_runtime::spawn` soumet la tâche
//! directement au runtime que Tauri gère en interne, sans dépendre du
//! contexte thread-local ambiant — contrairement à `tokio::spawn`, qui
//! panique avec `there is no reactor running` si appelé en dehors d'une
//! tâche déjà pilotée par un runtime Tokio. Voir le cahier technique,
//! section "Stabilisation", pour le détail de ce piège et son audit complet.
//!
//! Conséquence directe : le type retourné est `tauri::async_runtime::JoinHandle<T>`,
//! **pas** `tokio::task::JoinHandle<T>` — ce sont deux types distincts dans
//! l'API publique de Tauri 2 (erreur de compilation typique si l'un est
//! utilisé à la place de l'autre : `expected TokioJoinHandle<()>, found
//! JoinHandle<()>`). `ChannelHandle.task` est donc typé avec l'alias de
//! `tauri::async_runtime`, pas celui de `tokio`.
//!
//! ## Suivi d'état : source de vérité unique
//!
//! `statuses` est la source de vérité pour l'état de chaque chaîne connue
//! (interrogée par [`ConnectionManager::connected_channels`]) ; elle est
//! mise à jour par le même point de passage (`set_status`) qui émet aussi
//! l'évènement temps réel vers le frontend, pour qu'il n'y ait jamais de
//! divergence entre "ce que l'UI reçoit en direct" et "ce que l'UI voit en
//! rechargeant la liste".

use super::irc_client::{ChannelEvent, IrcChannelClient};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tauri::async_runtime::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
}

struct ChannelHandle {
    task: JoinHandle<()>,
    stop_tx: mpsc::Sender<()>,
}

/// Évènement global émis vers l'application (au-delà des messages de chat) :
/// utile pour piloter l'indicateur d'état par chaîne dans l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ManagerEvent {
    StatusChanged {
        channel: String,
        status: ConnectionStatus,
    },
    ChatMessage(crate::twitch::message::ChatMessage),
    Notice {
        channel: String,
        text: String,
    },
}

/// Met à jour la source de vérité (`statuses`) *et* notifie le frontend, en
/// un seul point de passage — pour que les deux ne puissent jamais diverger.
/// Fonction libre (pas de méthode `&self`) car appelée depuis l'intérieur
/// de la tâche `'static` spawnée, qui ne peut pas emprunter `&ConnectionManager`.
async fn set_status(
    statuses: &Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    events_tx: &mpsc::UnboundedSender<ManagerEvent>,
    channel: &str,
    status: ConnectionStatus,
) {
    statuses.lock().await.insert(channel.to_string(), status);
    events_tx
        .send(ManagerEvent::StatusChanged {
            channel: channel.to_string(),
            status,
        })
        .ok();
}

pub struct ConnectionManager {
    channels: Arc<Mutex<HashMap<String, ChannelHandle>>>,
    statuses: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    events_tx: mpsc::UnboundedSender<ManagerEvent>,
    bot_login: String,
    oauth_token_provider: Arc<dyn Fn() -> Option<String> + Send + Sync>,
}

impl ConnectionManager {
    /// `oauth_token_provider` est appelé à chaque (re)connexion pour obtenir
    /// un token frais (permet au module d'auth de rafraîchir le token entre
    /// deux tentatives sans que ce module ait besoin de connaître la logique
    /// OAuth).
    pub fn new(
        bot_login: String,
        oauth_token_provider: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    ) -> (Self, mpsc::UnboundedReceiver<ManagerEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Self {
                channels: Arc::new(Mutex::new(HashMap::new())),
                statuses: Arc::new(Mutex::new(HashMap::new())),
                events_tx,
                bot_login,
                oauth_token_provider,
            },
            events_rx,
        )
    }

    /// Retourne l'état actuellement connu de toutes les chaînes (y compris
    /// celles déconnectées, dont l'entrée est conservée avec le statut
    /// `Disconnected` plutôt que supprimée — pour que l'interface, en cas de
    /// rechargement, distingue "jamais connectée" de "déconnectée").
    pub async fn connected_channels(&self) -> Vec<(String, ConnectionStatus)> {
        self.statuses
            .lock()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Démarre la connexion à une chaîne. Idempotent : si une tâche est
    /// déjà active pour cette chaîne, ne fait rien (retourne `Ok(())`).
    pub async fn connect(&self, channel_login: &str) -> anyhow::Result<()> {
        let channel_login = channel_login.to_lowercase();
        {
            let channels = self.channels.lock().await;
            if channels.contains_key(&channel_login) {
                return Ok(());
            }
        }

        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let events_tx = self.events_tx.clone();
        let statuses = self.statuses.clone();
        let channels_registry = self.channels.clone();
        let bot_login = self.bot_login.clone();
        let token_provider = self.oauth_token_provider.clone();
        let channel_for_task = channel_login.clone();

        let task = tauri::async_runtime::spawn(async move {
            let mut backoff_secs: u64 = 1;
            const MAX_BACKOFF_SECS: u64 = 60;

            loop {
                let Some(token) = token_provider() else {
                    set_status(&statuses, &events_tx, &channel_for_task, ConnectionStatus::Disconnected).await;
                    // Personne n'appellera `disconnect()` pour cette chaîne
                    // (elle n'a jamais réussi à démarrer) : c'est à la tâche
                    // elle-même de se retirer du registre `channels`, sinon
                    // `connect()` la croira indéfiniment active (son test
                    // d'idempotence `contains_key` resterait bloqué à `true`)
                    // et un nouvel essai après authentification ne ferait
                    // jamais rien — c'est exactement le bug initialement
                    // observé : une chaîne ajoutée avant connexion Twitch
                    // restait bloquée en permanence, y compris après login.
                    channels_registry.lock().await.remove(&channel_for_task);
                    break;
                };

                set_status(&statuses, &events_tx, &channel_for_task, ConnectionStatus::Connecting).await;

                let (chan_tx, mut chan_rx) = mpsc::unbounded_channel::<ChannelEvent>();
                let client = IrcChannelClient::new(channel_for_task.clone(), token, bot_login.clone());

                let run_future = client.run(chan_tx);
                tokio::pin!(run_future);

                let bridge_events_tx = events_tx.clone();
                let bridge_statuses = statuses.clone();
                let bridge_channel = channel_for_task.clone();
                let bridge = async move {
                    while let Some(evt) = chan_rx.recv().await {
                        match evt {
                            ChannelEvent::Connected => {
                                set_status(
                                    &bridge_statuses,
                                    &bridge_events_tx,
                                    &bridge_channel,
                                    ConnectionStatus::Connected,
                                )
                                .await;
                            }
                            ChannelEvent::Disconnected { reason } => {
                                bridge_events_tx
                                    .send(ManagerEvent::Notice {
                                        channel: bridge_channel.clone(),
                                        text: reason,
                                    })
                                    .ok();
                            }
                            ChannelEvent::Message(msg) => {
                                bridge_events_tx.send(ManagerEvent::ChatMessage(msg)).ok();
                            }
                            ChannelEvent::Notice(text) => {
                                bridge_events_tx
                                    .send(ManagerEvent::Notice {
                                        channel: bridge_channel.clone(),
                                        text,
                                    })
                                    .ok();
                            }
                        }
                    }
                };
                tokio::pin!(bridge);

                // Délai d'inactivité réseau : si aucun message/PING n'est
                // reçu pendant cette durée, on considère la connexion
                // silencieusement morte (coupure réseau sans fermeture
                // propre du socket, ex: changement de Wi-Fi, veille système)
                // et on force une reconnexion plutôt que d'attendre
                // indéfiniment un flux qui ne reviendra jamais. Twitch émet
                // un PING serveur toutes les ~5 minutes ; 6 minutes laisse
                // une marge raisonnable sans fausse alerte.
                const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6 * 60);

                tokio::select! {
                    _ = stop_rx.recv() => {
                        break;
                    }
                    result = tokio::time::timeout(IDLE_TIMEOUT, &mut run_future) => {
                        if result.is_err() {
                            send_notice(&events_tx, &channel_for_task, "Connexion inactive depuis plus de 6 minutes, reconnexion forcée");
                        }
                        // Sinon : la connexion s'est terminée d'elle-même
                        // (erreur réseau, refus serveur, etc.) — on continue
                        // vers la boucle de reconnexion ci-dessous.
                    }
                    _ = &mut bridge => {}
                }

                set_status(&statuses, &events_tx, &channel_for_task, ConnectionStatus::Reconnecting).await;

                tokio::select! {
                    _ = stop_rx.recv() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                }
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
            }
        });

        self.channels
            .lock()
            .await
            .insert(channel_login.clone(), ChannelHandle { task, stop_tx });
        self.statuses
            .lock()
            .await
            .insert(channel_login, ConnectionStatus::Connecting);

        Ok(())
    }

    /// Déconnecte une chaîne et **attend la terminaison effective** de sa
    /// tâche avant d'annoncer l'état `Disconnected`.
    ///
    /// Ce point est important : `task.abort()` seul ne suffit pas à
    /// garantir l'ordre des évènements. `abort()` demande l'annulation mais
    /// ne bloque pas — si la tâche est, au même instant, en train
    /// d'exécuter (sur un autre thread du pool Tokio) la branche `bridge`
    /// qui vient de recevoir un `ChannelEvent::Connected`, elle peut encore
    /// émettre un `StatusChanged { Connected }` *après* que ce code envoie
    /// `StatusChanged { Disconnected }`, en fonction de l'ordonnancement.
    /// Le frontend afficherait alors une chaîne qui « se reconnecte toute
    /// seule » juste après un clic sur Déconnecter. En attendant
    /// explicitement `handle.task` après `abort()`, on garantit que la
    /// tâche est totalement arrêtée — donc qu'elle ne peut plus rien
    /// émettre — avant d'envoyer l'état final.
    pub async fn disconnect(&self, channel_login: &str) {
        let channel_login = channel_login.to_lowercase();
        if let Some(handle) = self.channels.lock().await.remove(&channel_login) {
            handle.stop_tx.send(()).await.ok();
            handle.task.abort();
            let _ = handle.task.await; // attend la terminaison réelle (Err attendu : tâche annulée)

            set_status(&self.statuses, &self.events_tx, &channel_login, ConnectionStatus::Disconnected).await;
        }
    }

    pub async fn disconnect_all(&self) {
        let logins: Vec<String> = self.channels.lock().await.keys().cloned().collect();
        for login in logins {
            self.disconnect(&login).await;
        }
    }
}

/// Petit utilitaire pour émettre une `Notice` sans dupliquer le `.send().ok()`
/// à chaque site d'appel.
fn send_notice(events_tx: &mpsc::UnboundedSender<ManagerEvent>, channel: &str, text: &str) {
    events_tx
        .send(ManagerEvent::Notice {
            channel: channel.to_string(),
            text: text.to_string(),
        })
        .ok();
}
