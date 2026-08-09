//! Commandes Tauri pour l'onglet "Paramètres" : configuration complète,
//! export/import JSON, réinitialisation. Des setters granulaires par
//! section sont également exposés pour éviter qu'une section (ex: filtres)
//! n'écrase accidentellement une autre section modifiée en parallèle par
//! un autre onglet de l'interface.

use crate::config::{
    AntiSpamConfig, AppConfig, AudioConfig, FiltersConfig, GeneralConfig, OverlayConfig, TtsConfig,
    TwitchConfig,
};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config.get()
}

/// Enregistre le Client ID Twitch saisi par l'utilisateur dans l'onglet
/// "Connexions Twitch". Validation stricte côté backend (jamais uniquement
/// côté frontend, qui peut toujours être contourné) : un identifiant vide
/// ou uniquement composé d'espaces est rejeté plutôt que silencieusement
/// accepté, ce qui produirait plus tard une erreur Twitch peu claire au
/// moment de la connexion plutôt qu'un message immédiat et compréhensible.
#[tauri::command]
pub fn update_twitch_config(
    client_id: String,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    let trimmed = client_id.trim();
    if trimmed.is_empty() {
        return Err(crate::error::AppError::InvalidConfig(
            "Le Client ID Twitch ne peut pas être vide.".into(),
        ));
    }
    state.config.update(|cfg| {
        cfg.twitch = TwitchConfig {
            client_id: Some(trimmed.to_string()),
        }
    })
}

#[tauri::command]
pub fn update_tts_config(
    tts: TtsConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| cfg.tts = tts)
}

#[tauri::command]
pub fn update_audio_config(
    audio: AudioConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| cfg.audio = audio)
}

#[tauri::command]
pub fn update_filters_config(
    filters: FiltersConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| cfg.filters = filters)
}

#[tauri::command]
pub fn update_anti_spam_config(
    anti_spam: AntiSpamConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| cfg.anti_spam = anti_spam)
}

#[tauri::command]
pub fn update_overlay_config(
    overlay: OverlayConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| cfg.overlay = overlay)
}

#[tauri::command]
pub fn update_general_config(
    general: GeneralConfig,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if general.launch_on_system_startup {
        autolaunch.enable().ok();
    } else {
        autolaunch.disable().ok();
    }
    state.config.update(|cfg| cfg.general = general)
}

#[tauri::command]
pub fn set_user_voice_assignment(
    login: String,
    voice_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| match voice_id {
        Some(v) => {
            cfg.voice_assignments.per_user.insert(login.to_lowercase(), v);
        }
        None => {
            cfg.voice_assignments.per_user.remove(&login.to_lowercase());
        }
    })
}

#[tauri::command]
pub fn set_role_voice_assignment(
    role: String,
    voice_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.update(|cfg| match voice_id {
        Some(v) => {
            cfg.voice_assignments.per_role.insert(role, v);
        }
        None => {
            cfg.voice_assignments.per_role.remove(&role);
        }
    })
}

#[tauri::command]
pub async fn export_config(
    destination_path: String,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    state.config.export_to(std::path::Path::new(&destination_path))
}

#[tauri::command]
pub async fn import_config(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<AppConfig, crate::error::AppError> {
    state.config.import_from(std::path::Path::new(&source_path))
}

#[tauri::command]
pub fn reset_config(state: State<'_, AppState>) -> Result<AppConfig, crate::error::AppError> {
    state.config.reset_to_defaults()
}
