//! File d'attente de synthèse vocale.
//!
//! Conçue comme une tâche de fond unique consommant un canal `mpsc` de
//! [`QueuedMessage`], garantissant une lecture **séquentielle** (un seul
//! message lu à la fois, dans l'ordre d'arrivée) quel que soit le nombre de
//! chaînes connectées simultanément — élément essentiel pour l'intelligibilité
//! de la lecture vocale (lire deux messages en parallèle serait inutilisable).
//!
//! Pipeline par message : sélection de la voix -> synthèse Piper -> lecture
//! audio -> émission d'évènements (frontend + overlay) -> statistiques.

use crate::audio::AudioPlayer;
use crate::config::{ConfigStore, VoiceAssignments};
use crate::tts::language::detect_language_code;
use crate::tts::piper::PiperEngine;
use crate::twitch::message::{ChatMessage, TwitchRole};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct QueuedMessage {
    pub message: ChatMessage,
    /// Nombre d'occurrences si regroupé par l'anti-spam (ex: "x3"), sinon 1.
    pub occurrence_count: u32,
}

/// Évènement émis à chaque étape de lecture, consommé par le frontend
/// (mise à jour de l'UI en temps réel) et par le serveur d'overlay.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum TtsPlaybackEvent {
    Started {
        message_id: String,
        display_name: String,
        text: String,
        voice_id: String,
    },
    Finished {
        message_id: String,
        duration_ms: u64,
    },
    QueueSizeChanged {
        size: usize,
    },
    Error {
        message_id: String,
        error: String,
    },
}

pub struct TtsQueue {
    sender: mpsc::Sender<QueuedMessage>,
}

impl TtsQueue {
    /// Démarre la tâche de fond de traitement de la file. Retourne un
    /// handle léger (`TtsQueue`) que les autres modules utilisent pour
    /// pousser des messages sans connaître les détails d'implémentation.
    pub fn spawn(
        piper: Arc<PiperEngine>,
        audio_player: Arc<AudioPlayer>,
        config_store: ConfigStore,
        db: Arc<crate::db::Repository>,
        session_stats: Arc<crate::stats::SessionStats>,
        events_tx: broadcast::Sender<TtsPlaybackEvent>,
    ) -> Self {
        let max_queue_size = config_store.get().tts.max_queue_size.max(1);
        let (tx, mut rx) = mpsc::channel::<QueuedMessage>(max_queue_size);

        tauri::async_runtime::spawn(async move {
            while let Some(queued) = rx.recv().await {
                events_tx
                    .send(TtsPlaybackEvent::QueueSizeChanged { size: rx.len() })
                    .ok();

                let config = config_store.get();
                let voice_id = Self::select_voice(&queued.message, &config.tts, &config.voice_assignments);

                let mut text_to_speak = queued.message.text_for_tts.clone();
                if queued.occurrence_count > 1 {
                    text_to_speak = format!("{text_to_speak} (x{})", queued.occurrence_count);
                }
                if config.tts.read_username_before_message {
                    text_to_speak = format!("{} dit : {text_to_speak}", queued.message.display_name);
                }

                events_tx
                    .send(TtsPlaybackEvent::Started {
                        message_id: queued.message.id.clone(),
                        display_name: queued.message.display_name.clone(),
                        text: queued.message.text.clone(),
                        voice_id: voice_id.clone(),
                    })
                    .ok();

                let started_at = std::time::Instant::now();

                // Coupe-circuit : si Piper reste bloqué (modèle corrompu,
                // process zombie, bug interne) sans jamais retourner, la
                // file entière — donc toute lecture du chat pour le reste
                // de la session — resterait gelée indéfiniment sur ce seul
                // message, sans le moindre message d'erreur visible. Un
                // timeout généreux transforme ce blocage silencieux en une
                // erreur explicite, et libère la file pour le message suivant.
                const SYNTHESIS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
                const PLAYBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

                let synthesis_result = tokio::time::timeout(
                    SYNTHESIS_TIMEOUT,
                    piper.synthesize(&text_to_speak, &voice_id, 1.0 / config.tts.rate.max(0.1)),
                )
                .await;

                match synthesis_result {
                    Err(_elapsed) => {
                        events_tx
                            .send(TtsPlaybackEvent::Error {
                                message_id: queued.message.id.clone(),
                                error: format!(
                                    "Le moteur Piper n'a pas répondu dans les {}s (process probablement bloqué)",
                                    SYNTHESIS_TIMEOUT.as_secs()
                                ),
                            })
                            .ok();
                    }
                    Ok(Err(e)) => {
                        events_tx
                            .send(TtsPlaybackEvent::Error {
                                message_id: queued.message.id.clone(),
                                error: e.to_string(),
                            })
                            .ok();
                    }
                    Ok(Ok(audio)) => {
                        let per_voice_volume = config
                            .audio
                            .per_voice_volume
                            .get(&voice_id)
                            .copied()
                            .unwrap_or(1.0);
                        let effective_volume =
                            config.audio.master_volume * config.tts.volume * per_voice_volume;

                        let playback_result = tokio::time::timeout(
                            PLAYBACK_TIMEOUT,
                            audio_player.play_pcm(audio.samples, audio.sample_rate, effective_volume, config.tts.pitch),
                        )
                        .await;

                        match playback_result {
                            Err(_elapsed) => {
                                events_tx
                                    .send(TtsPlaybackEvent::Error {
                                        message_id: queued.message.id.clone(),
                                        error: "Lecture audio anormalement longue, message annulé".into(),
                                    })
                                    .ok();
                            }
                            Ok(Err(e)) => {
                                events_tx
                                    .send(TtsPlaybackEvent::Error {
                                        message_id: queued.message.id.clone(),
                                        error: e.to_string(),
                                    })
                                    .ok();
                            }
                            Ok(Ok(())) => {}
                        }
                    }
                }

                let duration_ms = started_at.elapsed().as_millis() as u64;
                // Même remarque que dans `lib.rs::process_incoming_message` :
                // `rusqlite` est bloquant, on le déporte pour ne jamais
                // retarder le traitement du message TTS suivant dans la file.
                let db_for_stats = db.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    db_for_stats.add_reading_time(duration_ms as i64).ok();
                });
                session_stats.record_reading_time(duration_ms);
                events_tx
                    .send(TtsPlaybackEvent::Finished {
                        message_id: queued.message.id,
                        duration_ms,
                    })
                    .ok();
            }
        });

        Self { sender: tx }
    }

    /// Détermine la voix à utiliser, par ordre de priorité décroissante :
    /// 1. Voix assignée spécifiquement à l'utilisateur ;
    /// 2. Voix assignée à son rôle Twitch ;
    /// 3. Voix déterminée par détection automatique de langue (si activée) ;
    /// 4. Voix par défaut.
    fn select_voice(
        message: &ChatMessage,
        tts_config: &crate::config::TtsConfig,
        assignments: &VoiceAssignments,
    ) -> String {
        if let Some(voice) = assignments.per_user.get(&message.username_login) {
            return voice.clone();
        }

        if let Some(voice) = assignments.per_role.get(Self::role_key(message.role)) {
            return voice.clone();
        }

        if tts_config.auto_detect_language {
            if let Some(lang_code) = detect_language_code(&message.text_for_tts) {
                if let Some(voice) = tts_config.language_voice_map.get(lang_code) {
                    return voice.clone();
                }
            }
        }

        tts_config.default_voice_id.clone()
    }

    fn role_key(role: TwitchRole) -> &'static str {
        role.as_str()
    }

    /// Tente d'insérer un message dans la file. Retourne `false` si la file
    /// est pleine (le message est alors comptabilisé comme "ignoré" par
    /// l'appelant plutôt que de bloquer indéfiniment la réception du chat).
    pub fn try_enqueue(&self, item: QueuedMessage) -> bool {
        self.sender.try_send(item).is_ok()
    }
}
