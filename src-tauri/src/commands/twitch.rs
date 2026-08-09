//! Commandes Tauri pour la connexion/déconnexion des chaînes Twitch et la
//! consultation de leur état.

use crate::config::ChannelConfig;
use crate::state::AppState;
use crate::twitch::ConnectionStatus;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub async fn twitch_connect_channel(
    login: String,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    let login = login.to_lowercase();

    state.config.update(|cfg| {
        if !cfg.channels.iter().any(|c| c.login == login) {
            cfg.channels.push(ChannelConfig {
                login: login.clone(),
                enabled: true,
                auto_reconnect: true,
            });
        } else if let Some(c) = cfg.channels.iter_mut().find(|c| c.login == login) {
            c.enabled = true;
        }
    })?;

    state
        .connection_manager
        .connect(&login)
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))
}

#[tauri::command]
pub async fn twitch_disconnect_channel(
    login: String,
    state: State<'_, AppState>,
) -> Result<(), crate::error::AppError> {
    let login = login.to_lowercase();
    state.connection_manager.disconnect(&login).await;
    state.config.update(|cfg| {
        if let Some(c) = cfg.channels.iter_mut().find(|c| c.login == login) {
            c.enabled = false;
        }
    })?;
    Ok(())
}

/// Fusionne les chaînes persistées (`config.json`, survivent à un
/// redémarrage) avec l'état vivant du `ConnectionManager` (en mémoire,
/// réinitialisé à chaque lancement). Sans cette fusion, une chaîne
/// désactivée/déconnectée disparaissait purement et simplement de la
/// liste après un redémarrage de l'application — jusqu'à ce que
/// l'utilisateur retape manuellement son nom.
#[tauri::command]
pub async fn twitch_list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<(String, ConnectionStatus)>, crate::error::AppError> {
    let mut merged: HashMap<String, ConnectionStatus> = state
        .config
        .get()
        .channels
        .iter()
        .map(|c| (c.login.clone(), ConnectionStatus::Disconnected))
        .collect();

    for (login, status) in state.connection_manager.connected_channels().await {
        merged.insert(login, status);
    }

    Ok(merged.into_iter().collect())
}
