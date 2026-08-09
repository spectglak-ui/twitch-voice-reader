//! Synthèse vocale : moteur Piper, détection de langue, file de lecture.

pub mod installer;
pub mod language;
pub mod piper;
pub mod queue;

pub use installer::InstallProgress;
pub use piper::PiperEngine;
pub use queue::{QueuedMessage, TtsPlaybackEvent, TtsQueue};
