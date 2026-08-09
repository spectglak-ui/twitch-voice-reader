//! Gestion centralisée des erreurs de l'application.
//!
//! Toutes les erreurs métier transitent par [`AppError`], qui implémente
//! `serde::Serialize` afin de pouvoir être renvoyé directement au frontend
//! via les `Result<T, AppError>` des commandes Tauri (celles-ci sérialisent
//! automatiquement l'erreur en JSON côté JS).

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Erreur réseau : {0}")]
    Network(#[from] reqwest::Error),

    #[error("Erreur WebSocket : {0}")]
    WebSocket(String),

    #[error("Erreur base de données : {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Erreur pool de connexions DB : {0}")]
    Pool(#[from] r2d2::Error),

    #[error("Erreur de sérialisation JSON : {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Erreur E/S : {0}")]
    Io(#[from] std::io::Error),

    #[error("Authentification Twitch requise ou expirée")]
    AuthRequired,

    #[error("Échec de l'authentification Twitch : {0}")]
    AuthFailed(String),

    #[error("Moteur TTS (Piper) indisponible : {0}")]
    TtsUnavailable(String),

    #[error("Périphérique audio introuvable : {0}")]
    AudioDevice(String),

    #[error("Configuration invalide : {0}")]
    InvalidConfig(String),

    #[error("Chaîne déjà connectée : {0}")]
    AlreadyConnected(String),

    #[error("Chaîne non connectée : {0}")]
    NotConnected(String),

    #[error("Erreur interne : {0}")]
    Internal(String),
}

/// Implémentation manuelle de `Serialize` : on expose un objet `{ kind, message }`
/// stable pour le frontend TypeScript, plutôt que de dépendre du format `Display`.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let kind = match self {
            AppError::Network(_) => "network",
            AppError::WebSocket(_) => "websocket",
            AppError::Database(_) => "database",
            AppError::Pool(_) => "database",
            AppError::Serde(_) => "serde",
            AppError::Io(_) => "io",
            AppError::AuthRequired => "auth_required",
            AppError::AuthFailed(_) => "auth_failed",
            AppError::TtsUnavailable(_) => "tts_unavailable",
            AppError::AudioDevice(_) => "audio_device",
            AppError::InvalidConfig(_) => "invalid_config",
            AppError::AlreadyConnected(_) => "already_connected",
            AppError::NotConnected(_) => "not_connected",
            AppError::Internal(_) => "internal",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", kind)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
