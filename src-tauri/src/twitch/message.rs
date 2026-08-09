//! Parsing des lignes IRC brutes envoyées par `irc-ws.chat.twitch.tv`.
//!
//! Twitch étend IRCv3 avec des tags (`@badges=...;color=...;...`). On
//! parse manuellement plutôt que d'utiliser une crate IRC générique afin de
//! garder un contrôle total sur les tags spécifiques à Twitch (badges,
//! `tmi-sent-ts`, `first-msg`, `reply-parent-*`, etc.) sans dépendance
//! externe supplémentaire à maintenir.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rôle Twitch déduit des badges du message. Un utilisateur peut cumuler
/// plusieurs badges ; on retient le rôle le plus "élevé" pour les filtres
/// et l'attribution de voix par rôle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwitchRole {
    Viewer,
    Subscriber,
    Vip,
    Moderator,
    Broadcaster,
}

impl TwitchRole {
    /// Détermine le rôle le plus élevé à partir de la chaîne `badges` brute
    /// (ex: `"broadcaster/1,subscriber/12,premium/1"`).
    pub fn from_badges(badges_raw: &str) -> Self {
        let mut role = TwitchRole::Viewer;
        for badge in badges_raw.split(',') {
            let name = badge.split('/').next().unwrap_or("");
            role = match (name, role) {
                ("broadcaster", _) => return TwitchRole::Broadcaster,
                ("moderator", r) if r < TwitchRole::Moderator => TwitchRole::Moderator,
                ("vip", r) if r < TwitchRole::Vip => TwitchRole::Vip,
                ("subscriber" | "founder", r) if r < TwitchRole::Subscriber => {
                    TwitchRole::Subscriber
                }
                (_, r) => r,
            };
        }
        role
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TwitchRole::Viewer => "viewer",
            TwitchRole::Subscriber => "subscriber",
            TwitchRole::Vip => "vip",
            TwitchRole::Moderator => "moderator",
            TwitchRole::Broadcaster => "broadcaster",
        }
    }
}

// Ordre total pour pouvoir comparer les rôles (Viewer < Subscriber < ... < Broadcaster)
impl PartialOrd for TwitchRole {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TwitchRole {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(r: &TwitchRole) -> u8 {
            match r {
                TwitchRole::Viewer => 0,
                TwitchRole::Subscriber => 1,
                TwitchRole::Vip => 2,
                TwitchRole::Moderator => 3,
                TwitchRole::Broadcaster => 4,
            }
        }
        rank(self).cmp(&rank(other))
    }
}

/// Message de chat entièrement décodé, prêt à être filtré / mis en file TTS
/// / affiché dans l'interface. C'est le type "pivot" partagé entre le
/// backend et le frontend (miroir TypeScript dans `src/types/chat.ts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub channel: String,
    pub username_login: String,
    pub display_name: String,
    pub color: Option<String>,
    pub role: TwitchRole,
    pub badges: Vec<String>,
    pub text: String,
    /// Texte débarrassé des codes d'emotes Twitch pour la synthèse vocale.
    pub text_for_tts: String,
    pub is_emote_only: bool,
    pub is_action: bool, // message `/me`
    pub timestamp_ms: i64,
}

/// Représentation d'une ligne IRC brute avant décodage sémantique.
struct RawIrcLine<'a> {
    tags: HashMap<&'a str, &'a str>,
    command: &'a str,
    params: Vec<&'a str>,
}

fn parse_raw_line(line: &str) -> Option<RawIrcLine<'_>> {
    let mut rest = line.trim_end_matches(['\r', '\n']);
    let mut tags = HashMap::new();

    if let Some(stripped) = rest.strip_prefix('@') {
        let (tag_part, remainder) = stripped.split_once(' ')?;
        rest = remainder;
        for pair in tag_part.split(';') {
            if let Some((k, v)) = pair.split_once('=') {
                tags.insert(k, v);
            }
        }
    }

    // Ignore le préfixe `:nick!user@host` s'il est présent (non nécessaire,
    // les tags `display-name` / `login` suffisent pour PRIVMSG).
    if rest.starts_with(':') {
        if let Some((_, remainder)) = rest.split_once(' ') {
            rest = remainder;
        }
    }

    let mut parts = rest.splitn(2, " :");
    let head = parts.next()?;
    let trailing = parts.next();

    let mut params: Vec<&str> = head.split_whitespace().collect();
    let command = if params.is_empty() {
        return None;
    } else {
        params.remove(0)
    };
    if let Some(trailing) = trailing {
        params.push(trailing);
    }

    Some(RawIrcLine {
        tags,
        command,
        params,
    })
}

/// Retire les liens (http/https) d'un texte — utilisé par le filtre
/// "ignorer les liens" et pour nettoyer le texte envoyé au TTS.
pub fn strip_urls(text: &str) -> String {
    static URL_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"https?://\S+").expect("regex URL statique valide")
    });
    URL_RE.replace_all(text, "").trim().to_string()
}

/// Décode une ligne `PRIVMSG` en [`ChatMessage`]. Retourne `None` pour
/// toute autre commande IRC (PING, JOIN, NOTICE, etc.), gérées séparément
/// par [`super::irc_client`].
pub fn try_parse_privmsg(line: &str, channel_hint: &str) -> Option<ChatMessage> {
    let raw = parse_raw_line(line)?;
    if raw.command != "PRIVMSG" {
        return None;
    }

    let text_raw = *raw.params.last()?;
    let (is_action, text) = if let Some(inner) = text_raw
        .strip_prefix("\u{0001}ACTION ")
        .and_then(|s| s.strip_suffix('\u{0001}'))
    {
        (true, inner.to_string())
    } else {
        (false, text_raw.to_string())
    };

    let badges_raw = raw.tags.get("badges").copied().unwrap_or_default();
    let role = TwitchRole::from_badges(badges_raw);
    let badges: Vec<String> = badges_raw
        .split(',')
        .filter(|b| !b.is_empty())
        .map(|b| b.split('/').next().unwrap_or(b).to_string())
        .collect();

    let display_name = raw
        .tags
        .get("display-name")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| channel_hint.to_string());

    let username_login = display_name.to_lowercase();
    let is_emote_only = raw.tags.get("emote-only") == Some(&"1");

    Some(ChatMessage {
        id: raw
            .tags
            .get("id")
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        channel: channel_hint.to_string(),
        username_login,
        display_name,
        color: raw
            .tags
            .get("color")
            .filter(|c| !c.is_empty())
            .map(|s| s.to_string()),
        role,
        badges,
        text: text.clone(),
        text_for_tts: strip_urls(&text),
        is_emote_only,
        is_action,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

/// Retourne `true` si la ligne est un `PING` serveur, auquel cas le client
/// doit répondre immédiatement par `PONG` pour ne pas être déconnecté.
pub fn is_ping(line: &str) -> bool {
    line.starts_with("PING")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_privmsg() {
        let line = "@badges=moderator/1;color=#FF0000;display-name=ExampleUser;id=abc-123;emote-only=0 :exampleuser!exampleuser@exampleuser.tmi.twitch.tv PRIVMSG #somechannel :Hello world!";
        let msg = try_parse_privmsg(line, "somechannel").expect("devrait parser");
        assert_eq!(msg.display_name, "ExampleUser");
        assert_eq!(msg.role, TwitchRole::Moderator);
        assert_eq!(msg.text, "Hello world!");
        assert!(!msg.is_action);
    }

    #[test]
    fn strips_urls_for_tts() {
        assert_eq!(strip_urls("regarde ça https://example.com/x cool"), "regarde ça  cool".replace("  ", " ").trim());
    }

    #[test]
    fn detects_broadcaster_over_moderator() {
        let role = TwitchRole::from_badges("moderator/1,broadcaster/1");
        assert_eq!(role, TwitchRole::Broadcaster);
    }
}
