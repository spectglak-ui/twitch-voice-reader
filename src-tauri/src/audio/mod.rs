//! Lecture audio et gestion des périphériques de sortie.

pub mod devices;
pub mod player;

pub use devices::{list_output_devices, AudioDeviceInfo};
pub use player::AudioPlayer;
