//! Schéma de configuration de l'application.
//!
//! `AppConfig` est LA source de vérité persistée sur disque (JSON) et
//! exposée telle quelle au frontend. Toute nouvelle fonctionnalité doit
//! ajouter ses champs ici plutôt que de créer un fichier de config parallèle,
//! afin de garder un export/import JSON unique et cohérent.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Version du schéma, utilisée pour les migrations futures.
    pub schema_version: u32,
    /// `#[serde(default)]` : ajoutée après la première version distribuée
    /// du schéma. Sans cet attribut, charger un `config.json` existant
    /// (créé avant cette fonctionnalité) qui ne contient pas encore ce
    /// champ ferait échouer la désérialisation entière — et
    /// `ConfigStore::load` retomberait alors silencieusement sur
    /// `AppConfig::default()`, effaçant au passage TOUTES les autres
    /// préférences déjà enregistrées par l'utilisateur (chaînes, réglages
    /// TTS, filtres...). Tout nouveau champ ajouté au schéma doit suivre
    /// cette même règle.
    #[serde(default)]
    pub twitch: TwitchConfig,
    pub channels: Vec<ChannelConfig>,
    pub tts: TtsConfig,
    pub audio: AudioConfig,
    pub filters: FiltersConfig,
    pub anti_spam: AntiSpamConfig,
    pub voice_assignments: VoiceAssignments,
    pub overlay: OverlayConfig,
    pub general: GeneralConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            twitch: TwitchConfig::default(),
            channels: Vec::new(),
            tts: TtsConfig::default(),
            audio: AudioConfig::default(),
            filters: FiltersConfig::default(),
            anti_spam: AntiSpamConfig::default(),
            voice_assignments: VoiceAssignments::default(),
            overlay: OverlayConfig::default(),
            general: GeneralConfig::default(),
        }
    }
}

/// Identifiant client de l'application Twitch (voir `twitch::auth` pour la
/// justification du Device Code Flow — c'est une valeur **publique**,
/// jamais un secret, donc parfaitement sûre à stocker dans `config.json`
/// en clair, contrairement aux jetons OAuth qui vivent eux dans le
/// trousseau système, voir `twitch::token_store`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TwitchConfig {
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Nom de la chaîne Twitch (login, minuscules).
    pub login: String,
    pub enabled: bool,
    pub auto_reconnect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Chemin vers le binaire Piper (résolu automatiquement si vide).
    pub piper_binary_path: Option<String>,
    /// Voix par défaut (identifiant du modèle .onnx Piper).
    pub default_voice_id: String,
    pub volume: f32,   // 0.0 - 1.0
    pub rate: f32,     // 0.5 - 2.0 (multiplicateur de vitesse)
    pub pitch: f32,    // -1.0 - 1.0 (semi-tons relatifs, appliqué en post-traitement)
    pub auto_detect_language: bool,
    /// Correspondance langue détectée -> voix à utiliser.
    pub language_voice_map: HashMap<String, String>,
    pub read_username_before_message: bool,
    pub max_queue_size: usize,
}

impl Default for TtsConfig {
    fn default() -> Self {
        let mut language_voice_map = HashMap::new();
        language_voice_map.insert("fr".into(), "fr_FR-siwis-medium".into());
        language_voice_map.insert("en".into(), "en_US-lessac-medium".into());
        language_voice_map.insert("es".into(), "es_ES-davefx-medium".into());
        language_voice_map.insert("de".into(), "de_DE-thorsten-medium".into());
        language_voice_map.insert("it".into(), "it_IT-riccardo-x_low".into());

        Self {
            piper_binary_path: None,
            default_voice_id: "fr_FR-siwis-medium".into(),
            volume: 0.8,
            rate: 1.0,
            pitch: 0.0,
            auto_detect_language: false,
            language_voice_map,
            read_username_before_message: true,
            max_queue_size: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// `None` = périphérique de sortie par défaut du système.
    pub output_device_name: Option<String>,
    pub master_volume: f32,
    /// Volume par identifiant de voix (surcharge du volume maître).
    pub per_voice_volume: HashMap<String, f32>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device_name: None,
            master_volume: 1.0,
            per_voice_volume: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiltersConfig {
    pub min_length: usize,
    pub max_length: usize,
    pub ignore_emote_only_messages: bool,
    pub ignore_links: bool,
    pub blacklist_words: Vec<String>,
    pub whitelist_words: Vec<String>,
    /// Si non vide, SEULS les messages contenant un mot de cette liste sont lus.
    pub whitelist_mode_enabled: bool,
    pub ignored_users: Vec<String>,
    pub roles: RoleFilterConfig,
}

impl Default for FiltersConfig {
    fn default() -> Self {
        Self {
            min_length: 2,
            max_length: 300,
            ignore_emote_only_messages: true,
            ignore_links: false,
            blacklist_words: Vec::new(),
            whitelist_words: Vec::new(),
            whitelist_mode_enabled: false,
            ignored_users: Vec::new(),
            roles: RoleFilterConfig::default(),
        }
    }
}

/// Restreint la lecture vocale à certains rôles. Si tous les champs sont
/// `false`, tout le monde est lu (comportement par défaut).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleFilterConfig {
    pub subscribers_only: bool,
    pub vips_only: bool,
    pub moderators_only: bool,
    pub broadcaster_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiSpamConfig {
    pub enabled: bool,
    /// Nombre max de messages lus par minute, toutes chaînes confondues.
    pub max_messages_per_minute: u32,
    /// Fenêtre de temps (secondes) pendant laquelle deux messages identiques
    /// consécutifs sont regroupés au lieu d'être lus deux fois.
    pub duplicate_grouping_window_secs: u64,
    /// Nombre de répétitions similaires avant de couper (protection contre
    /// le spam de raid/copypasta).
    pub repetition_threshold: u32,
}

impl Default for AntiSpamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_messages_per_minute: 20,
            duplicate_grouping_window_secs: 10,
            repetition_threshold: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoiceAssignments {
    /// login Twitch (minuscules) -> id de voix Piper.
    pub per_user: HashMap<String, String>,
    /// rôle ("subscriber" | "vip" | "moderator" | "broadcaster") -> id de voix.
    pub per_role: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub enabled: bool,
    pub http_port: u16,
    pub show_avatar: bool,
    pub show_username: bool,
    pub animation: OverlayAnimation,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            http_port: 47831,
            show_avatar: true,
            show_username: true,
            animation: OverlayAnimation::Fade,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OverlayAnimation {
    Fade,
    SlideUp,
    Bounce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub start_minimized_to_tray: bool,
    pub launch_on_system_startup: bool,
    pub theme: AppTheme,
    pub locale: String,
    pub history_retention_days: u32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_minimized_to_tray: false,
            launch_on_system_startup: false,
            theme: AppTheme::Dark,
            locale: "fr".into(),
            history_retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppTheme {
    Dark,
    Light,
    System,
}
