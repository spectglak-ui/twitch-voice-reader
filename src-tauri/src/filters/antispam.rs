//! Protection anti-spam : détection de répétitions, regroupement des
//! messages identiques et limitation de débit (messages/minute).
//!
//! Implémenté comme une machine à états interne à faible empreinte mémoire
//! (fenêtres glissantes bornées), volontairement séparé du [`FilterEngine`]
//! (`rules.rs`) : les filtres sont des règles *statiques* par message, alors
//! que l'anti-spam est un état *dynamique* qui dépend de l'historique récent.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub struct AntiSpamEngine {
    /// Horodatages des derniers messages acceptés, toutes chaînes confondues,
    /// pour appliquer la limite messages/minute (fenêtre glissante de 60s).
    recent_accepted: VecDeque<Instant>,
    /// Dernier texte normalisé vu par utilisateur, avec compteur de
    /// répétitions consécutives et horodatage (pour le regroupement de
    /// doublons et la coupure après répétition excessive, ex: raid copypasta).
    last_message_by_user: HashMap<String, (String, u32, Instant)>,
}

pub enum AntiSpamVerdict {
    /// Le message doit être lu normalement.
    Allow,
    /// Le message est un doublon récent : on ne le relit pas, mais on
    /// incrémente un compteur visuel ("x3") côté interface/historique.
    GroupedDuplicate { occurrence_count: u32 },
    /// Le message dépasse le seuil de répétition autorisé (probable
    /// spam/raid) : on le rejette complètement de la lecture vocale.
    RepetitionThresholdExceeded,
    /// La limite de messages/minute est atteinte : mis en attente plutôt
    /// que perdu (voir `tts::queue`).
    RateLimited,
}

impl AntiSpamEngine {
    pub fn new() -> Self {
        Self {
            recent_accepted: VecDeque::new(),
            last_message_by_user: HashMap::new(),
        }
    }

    /// Normalise un texte pour la comparaison de doublons (minuscules,
    /// espaces multiples réduits) sans altérer le texte original lu par le TTS.
    fn normalize(text: &str) -> String {
        text.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub fn evaluate(
        &mut self,
        user_login: &str,
        text: &str,
        config: &crate::config::AntiSpamConfig,
    ) -> AntiSpamVerdict {
        if !config.enabled {
            return AntiSpamVerdict::Allow;
        }

        let now = Instant::now();
        let normalized = Self::normalize(text);
        let grouping_window = Duration::from_secs(config.duplicate_grouping_window_secs);

        // --- Détection de répétition / regroupement de doublons ---------
        let verdict = match self.last_message_by_user.get_mut(user_login) {
            Some((last_text, count, last_seen))
                if *last_text == normalized && now.duration_since(*last_seen) <= grouping_window =>
            {
                *count += 1;
                *last_seen = now;
                if *count >= config.repetition_threshold {
                    AntiSpamVerdict::RepetitionThresholdExceeded
                } else {
                    AntiSpamVerdict::GroupedDuplicate {
                        occurrence_count: *count,
                    }
                }
            }
            _ => {
                self.last_message_by_user
                    .insert(user_login.to_string(), (normalized, 1, now));
                AntiSpamVerdict::Allow
            }
        };

        if !matches!(verdict, AntiSpamVerdict::Allow) {
            return verdict;
        }

        // --- Limite de débit global (messages/minute) --------------------
        while let Some(front) = self.recent_accepted.front() {
            if now.duration_since(*front) > Duration::from_secs(60) {
                self.recent_accepted.pop_front();
            } else {
                break;
            }
        }

        if self.recent_accepted.len() as u32 >= config.max_messages_per_minute {
            return AntiSpamVerdict::RateLimited;
        }

        self.recent_accepted.push_back(now);
        AntiSpamVerdict::Allow
    }

    /// Nettoyage périodique de la table utilisateur (évite une croissance
    /// mémoire non bornée sur les chaînes à très fort trafic). À appeler
    /// depuis une tâche de fond, ex. toutes les 5 minutes.
    pub fn prune_stale_entries(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.last_message_by_user
            .retain(|_, (_, _, last_seen)| now.duration_since(*last_seen) <= max_age);
    }
}

impl Default for AntiSpamEngine {
    fn default() -> Self {
        Self::new()
    }
}
