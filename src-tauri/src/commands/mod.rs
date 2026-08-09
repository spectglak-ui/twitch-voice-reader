//! Toutes les commandes Tauri exposées au frontend (`invoke(...)`).
//! Regroupées par domaine fonctionnel pour rester lisibles à mesure que
//! l'application grossit.

pub mod auth;
pub mod config;
pub mod overlay;
pub mod stats;
pub mod tts;
pub mod twitch;
