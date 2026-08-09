//! Commandes Tauri pour l'overlay web (Browser Source OBS).

use crate::overlay::OverlayServer;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn overlay_start(state: State<'_, AppState>) -> Result<u16, crate::error::AppError> {
    let config = state.config.get().overlay;
    let mut guard = state.overlay_server.lock().await;

    if guard.is_some() {
        return Ok(config.http_port);
    }

    let server = OverlayServer::start(config.http_port, state.tts_events.clone())
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

    *guard = Some(server);
    Ok(config.http_port)
}

#[tauri::command]
pub async fn overlay_stop(state: State<'_, AppState>) -> Result<(), crate::error::AppError> {
    let mut guard = state.overlay_server.lock().await;
    if let Some(mut server) = guard.take() {
        server.stop();
    }
    Ok(())
}

#[tauri::command]
pub async fn overlay_is_running(state: State<'_, AppState>) -> Result<bool, crate::error::AppError> {
    Ok(state.overlay_server.lock().await.is_some())
}
