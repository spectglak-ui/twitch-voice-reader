//! État applicatif partagé, exposé aux commandes Tauri via `tauri::State`.
//!
//! Centralise les handles vers chaque sous-système (config, DB, Twitch,
//! TTS, audio, overlay). Toute commande a besoin d'un sous-ensemble de ces
//! handles ; ils sont tous `Clone` bon marché (`Arc`/wrappers internes) pour
//! pouvoir être capturés librement dans les tâches asynchrones.

use crate::audio::AudioPlayer;
use crate::config::ConfigStore;
use crate::db::Repository;
use crate::filters::AntiSpamEngine;
use crate::overlay::OverlayServer;
use crate::stats::SessionStats;
use crate::tts::{PiperEngine, TtsPlaybackEvent, TtsQueue};
use crate::twitch::ConnectionManager;
use parking_lot::Mutex as SyncMutex;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex as AsyncMutex};

pub struct AppState {
    pub config: ConfigStore,
    pub db: Arc<Repository>,
    pub connection_manager: Arc<ConnectionManager>,
    pub piper: Arc<PiperEngine>,
    pub audio_player: Arc<AudioPlayer>,
    pub tts_queue: Arc<AsyncMutex<Option<TtsQueue>>>,
    pub anti_spam: Arc<SyncMutex<AntiSpamEngine>>,
    pub session_stats: Arc<SessionStats>,
    pub overlay_server: Arc<AsyncMutex<Option<OverlayServer>>>,
    /// Bus d'évènements de lecture TTS partagé entre : l'émission vers la
    /// fenêtre principale (mise à jour temps réel de l'UI) et le serveur
    /// d'overlay (Browser Source OBS). `broadcast` permet un nombre
    /// arbitraire d'abonnés indépendants sans coupler leurs cycles de vie.
    pub tts_events: Arc<broadcast::Sender<TtsPlaybackEvent>>,
}
