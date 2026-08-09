//! Commandes Tauri pour l'onglet "Voix et TTS" : liste des voix installées,
//! bouton de test vocal, liste des périphériques de sortie audio.

use crate::audio::{list_output_devices, AudioDeviceInfo};
use crate::state::AppState;
use tauri::{Emitter, State};

#[tauri::command]
pub fn tts_list_installed_voices(state: State<'_, AppState>) -> Vec<String> {
    state.piper.list_installed_voices()
}

#[tauri::command]
pub async fn tts_check_installation(
    state: State<'_, AppState>,
) -> Result<String, crate::error::AppError> {
    state.piper.check_installation().await
}

/// Déclenche (ou relance) l'installation automatique de Piper. Contrairement
/// à `tts_check_installation` (purement diagnostique), cette commande
/// **agit** : télécharge le binaire et/ou la voix par défaut si absents.
/// Idempotente — si tout est déjà prêt, retourne immédiatement. Émet des
/// évènements `piper://install-progress` tout au long du processus pour
/// que l'interface puisse afficher une progression réelle plutôt qu'une
/// simple attente silencieuse.
#[tauri::command]
pub async fn tts_ensure_installed(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    let progress_app = app.clone();
    state
        .piper
        .ensure_ready(move |progress| {
            progress_app.emit("piper://install-progress", &progress).ok();
        })
        .await
}

/// Bouton "Tester la voix" : synthétise et joue immédiatement un texte
/// donné, en court-circuitant la file d'attente principale (test isolé,
/// ne doit pas être affecté par l'anti-spam ni par la limite de débit).
#[tauri::command]
pub async fn tts_test_voice(
    text: String,
    voice_id: String,
    volume: f32,
    rate: f32,
    pitch: f32,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    let audio = state
        .piper
        .synthesize(&text, &voice_id, 1.0 / rate.max(0.1))
        .await?;

    let player = &state.audio_player;
    player.play_pcm(audio.samples, audio.sample_rate, volume, pitch).await
}

#[tauri::command]
pub fn audio_list_output_devices() -> Vec<AudioDeviceInfo> {
    list_output_devices()
}

#[tauri::command]
pub async fn audio_switch_output_device(
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    state.audio_player.switch_device(device_name).await
}
