//! Filtrage de contenu et protection anti-spam.

pub mod antispam;
pub mod rules;

pub use antispam::{AntiSpamEngine, AntiSpamVerdict};
pub use rules::{FilterEngine, FilterVerdict, RejectionReason};
