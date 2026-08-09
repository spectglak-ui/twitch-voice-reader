//! Moteur de règles déterminant si un message doit être lu à voix haute.
//!
//! Les règles sont évaluées dans un ordre précis, du filtre le "moins cher"
//! au "plus cher", afin de sortir au plus vite (`short-circuit`) sur les cas
//! les plus fréquents (utilisateur ignoré, longueur hors bornes) avant les
//! vérifications textuelles plus coûteuses (regex liens, listes de mots).

use crate::config::FiltersConfig;
use crate::twitch::message::ChatMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterVerdict {
    Accepted,
    Rejected(RejectionReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    UserIgnored,
    TooShort,
    TooLong,
    EmoteOnly,
    ContainsLink,
    Blacklisted,
    NotWhitelisted,
    RoleNotAllowed,
}

pub struct FilterEngine;

impl FilterEngine {
    pub fn evaluate(message: &ChatMessage, config: &FiltersConfig) -> FilterVerdict {
        use FilterVerdict::*;
        use RejectionReason::*;

        let login_lower = message.username_login.to_lowercase();
        if config
            .ignored_users
            .iter()
            .any(|u| u.to_lowercase() == login_lower)
        {
            return Rejected(UserIgnored);
        }

        if !Self::role_allowed(message, config) {
            return Rejected(RoleNotAllowed);
        }

        let len = message.text_for_tts.chars().count();
        if len < config.min_length {
            return Rejected(TooShort);
        }
        if config.max_length > 0 && len > config.max_length {
            return Rejected(TooLong);
        }

        if config.ignore_emote_only_messages && message.is_emote_only {
            return Rejected(EmoteOnly);
        }

        if config.ignore_links && message.text != message.text_for_tts {
            // Le texte contenait une URL retirée par `strip_urls`.
            return Rejected(ContainsLink);
        }

        let text_lower = message.text.to_lowercase();

        if config
            .blacklist_words
            .iter()
            .any(|w| !w.is_empty() && text_lower.contains(&w.to_lowercase()))
        {
            return Rejected(Blacklisted);
        }

        if config.whitelist_mode_enabled
            && !config.whitelist_words.is_empty()
            && !config
                .whitelist_words
                .iter()
                .any(|w| !w.is_empty() && text_lower.contains(&w.to_lowercase()))
        {
            return Rejected(NotWhitelisted);
        }

        Accepted
    }

    /// Chaque case à cocher ("abonnés", "VIP", "modérateurs", "broadcaster")
    /// est indépendante : on vérifie la présence du badge correspondant
    /// plutôt que le rôle dominant unique, car un spectateur peut être VIP
    /// sans être modérateur (et inversement) — les deux ensembles ne sont
    /// pas hiérarchiquement inclusifs sur Twitch.
    fn role_allowed(message: &ChatMessage, config: &FiltersConfig) -> bool {
        let roles = &config.roles;
        let any_restriction_active =
            roles.subscribers_only || roles.vips_only || roles.moderators_only || roles.broadcaster_only;

        if !any_restriction_active {
            return true; // aucune restriction : tout le monde est lu
        }

        let has_badge = |names: &[&str]| {
            message
                .badges
                .iter()
                .any(|b| names.iter().any(|n| b.eq_ignore_ascii_case(n)))
        };

        (roles.broadcaster_only && has_badge(&["broadcaster"]))
            || (roles.moderators_only && has_badge(&["moderator"]))
            || (roles.vips_only && has_badge(&["vip"]))
            || (roles.subscribers_only && has_badge(&["subscriber", "founder"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twitch::message::TwitchRole;

    fn sample_message(text: &str) -> ChatMessage {
        ChatMessage {
            id: "1".into(),
            channel: "test".into(),
            username_login: "viewer1".into(),
            display_name: "Viewer1".into(),
            color: None,
            role: TwitchRole::Viewer,
            badges: vec![],
            text: text.into(),
            text_for_tts: text.into(),
            is_emote_only: false,
            is_action: false,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn rejects_blacklisted_word() {
        let mut config = FiltersConfig::default();
        config.blacklist_words.push("spam".into());
        let msg = sample_message("ceci est du SPAM détecté");
        assert_eq!(
            FilterEngine::evaluate(&msg, &config),
            FilterVerdict::Rejected(RejectionReason::Blacklisted)
        );
    }

    #[test]
    fn accepts_normal_message() {
        let config = FiltersConfig::default();
        let msg = sample_message("bonjour le chat !");
        assert_eq!(FilterEngine::evaluate(&msg, &config), FilterVerdict::Accepted);
    }
}
