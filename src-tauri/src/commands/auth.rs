//! Commandes Tauri liées à l'authentification Twitch.
//!
//! Le flux complet (démarrage + polling) est piloté depuis une seule
//! commande : le frontend appelle `twitch_start_login`, affiche
//! immédiatement `verification_uri` / `user_code`, puis écoute les
//! évènements `auth://polling`, `auth://completed` et `auth://failed`
//! émis par la tâche de fond ci-dessous.

use crate::state::AppState;
use crate::twitch::auth::{is_client_id_configured, resolve_client_id, DeviceCodeResponse, TwitchAuthClient};
use crate::twitch::TokenStore;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn twitch_start_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DeviceCodeResponse, crate::error::AppError> {
    // Résolu à chaque tentative (config utilisateur -> variable d'env ->
    // placeholder) plutôt que figé au démarrage : voir la doc de
    // `resolve_client_id`. Validé explicitement ici plutôt que de laisser
    // Twitch renvoyer une erreur HTTP peu claire pour un Client ID absent.
    let client_id = resolve_client_id(&state.config.get().twitch);
    if !is_client_id_configured(&client_id) {
        return Err(crate::error::AppError::InvalidConfig(
            "Aucun Client ID Twitch configuré. Renseignez-le dans l'onglet \
             « Connexions Twitch » (section Configuration Twitch) avant de vous connecter."
                .into(),
        ));
    }

    let client = TwitchAuthClient::new(client_id.clone());
    let device_response = client.start_device_flow().await?;

    let device_for_task = device_response.clone();
    let app_for_task = app.clone();
    let connection_manager = state.connection_manager.clone();
    let config = state.config.clone();

    tauri::async_runtime::spawn(async move {
        let polling_client = TwitchAuthClient::new(client_id);
        let app_for_tick = app_for_task.clone();

        let result = polling_client
            .poll_for_tokens(&device_for_task, move || {
                app_for_tick.emit("auth://polling", ()).ok();
            })
            .await;

        match result {
            Ok(tokens) => {
                if let Err(e) = TokenStore::save(&tokens) {
                    app_for_task.emit("auth://failed", e.to_string()).ok();
                    return;
                }
                app_for_task.emit("auth://completed", &tokens.login).ok();

                // Reconnecte automatiquement les chaînes déjà configurées
                // avec ce nouveau jeton fraîchement obtenu.
                for channel in config.get().channels.iter().filter(|c| c.enabled) {
                    connection_manager.connect(&channel.login).await.ok();
                }
            }
            Err(e) => {
                app_for_task.emit("auth://failed", e.to_string()).ok();
            }
        }
    });

    Ok(device_response)
}

#[tauri::command]
pub async fn twitch_logout(state: State<'_, AppState>) -> Result<(), crate::error::AppError> {
    if let Some(tokens) = TokenStore::load_last() {
        TokenStore::clear(&tokens.login)?;
    }
    state.connection_manager.disconnect_all().await;
    Ok(())
}

#[tauri::command]
pub async fn twitch_current_account() -> Result<Option<String>, crate::error::AppError> {
    Ok(TokenStore::load_last().map(|t| t.login))
}
