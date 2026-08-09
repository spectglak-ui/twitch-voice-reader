//! Énumération des périphériques de sortie audio du système, utilisée par
//! le sélecteur de périphérique dans l'onglet "Voix et TTS".

use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok());

    let Ok(devices) = host.output_devices() else {
        return Vec::new();
    };

    devices
        .filter_map(|d| d.name().ok())
        .map(|name| AudioDeviceInfo {
            is_default: Some(&name) == default_name.as_ref(),
            name,
        })
        .collect()
}
