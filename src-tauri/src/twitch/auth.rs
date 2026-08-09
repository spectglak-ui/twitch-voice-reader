//! Authentification Twitch.
//!
//! ## Choix technique : Device Code Grant Flow
//!
//! Twitch ne supporte pas PKCE (confirmé sur le forum développeurs Twitch,
//! la demande est ouverte depuis 2019 et toujours sans suite). Pour une
//! application de bureau distribuée publiquement, les flux disponibles sont :
//!
//! | Flux                     | Nécessite un secret embarqué | Nécessite un serveur local | UX |
//! |---------------------------|:---:|:---:|---|
//! | Authorization Code        | Oui (fuite garantie si embarqué dans le binaire) | Oui (listener HTTP local + navigateur) | Bonne mais complexe |
//! | Implicit                  | Non | Non (redirect_uri custom) | Dépréciée par Twitch, token dans l'URL |
//! | **Device Code (retenu)**  | **Non** | **Non** | L'utilisateur ouvre `verification_uri`, saisit un code à 8 caractères |
//!
//! Le Device Code Flow ne nécessite ni secret client, ni serveur HTTP local,
//! ni schéma d'URI personnalisé — ce qui élimine toute une classe de
//! problèmes multiplateforme (association de protocole sur Windows/macOS/Linux).
//! Contrepartie : l'utilisateur doit copier un code dans son navigateur au
//! lieu d'un simple clic "Autoriser". C'est un compromis acceptable et
//! largement utilisé (CLI GitHub, GitHub Copilot, etc.).
//!
//! Le token d'accès expire après ~4h ; le refresh token est utilisé pour
//! obtenir un nouveau token sans ré-intervention de l'utilisateur.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const TWITCH_OAUTH_BASE: &str = "https://id.twitch.tv/oauth2";

/// Valeur de repli si ni la configuration utilisateur ni la variable
/// d'environnement `TWITCH_CLIENT_ID` ne fournissent d'identifiant — sert
/// uniquement à produire un message d'erreur explicite plutôt qu'une
/// chaîne vide silencieusement envoyée à l'API Twitch.
pub const CLIENT_ID_PLACEHOLDER: &str = "REPLACE_WITH_YOUR_TWITCH_CLIENT_ID";

/// Résout le Client ID Twitch à utiliser, par ordre de priorité :
/// 1. Valeur enregistrée par l'utilisateur dans l'onglet "Connexions
///    Twitch" (persistée dans `config.json`, champ `twitch.client_id`) ;
/// 2. Variable d'environnement `TWITCH_CLIENT_ID` (utile en développement
///    et en CI, voir `docs/GUIDE_COMPILATION.md`) ;
/// 3. Placeholder — traité comme "non configuré" par les appelants.
///
/// Recalculée à chaque tentative de connexion plutôt que figée une fois au
/// démarrage : sans ça, enregistrer un nouveau Client ID depuis l'interface
/// n'aurait d'effet qu'après un redémarrage complet de l'application, ce
/// qui serait déroutant (l'utilisateur vient de cliquer "Enregistrer").
pub fn resolve_client_id(twitch_config: &crate::config::TwitchConfig) -> String {
    if let Some(configured) = twitch_config.client_id.as_ref() {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    std::env::var("TWITCH_CLIENT_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| CLIENT_ID_PLACEHOLDER.to_string())
}

pub fn is_client_id_configured(client_id: &str) -> bool {
    client_id != CLIENT_ID_PLACEHOLDER && !client_id.trim().is_empty()
}

/// Scopes nécessaires : lecture du chat IRC + accès Helix pour badges/avatars.
pub const REQUIRED_SCOPES: &[&str] = &["chat:read", "user:read:chat"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: i64,
    pub login: String,
    pub user_id: String,
}

pub struct TwitchAuthClient {
    http: reqwest::Client,
    client_id: String,
}

impl TwitchAuthClient {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id: client_id.into(),
        }
    }

    /// Étape 1 : demande un couple (device_code, user_code) à Twitch.
    /// Le frontend affiche `user_code` et `verification_uri` à l'utilisateur.
    pub async fn start_device_flow(&self) -> AppResult<DeviceCodeResponse> {
        let resp = self
            .http
            .post(format!("{TWITCH_OAUTH_BASE}/device"))
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scopes", &REQUIRED_SCOPES.join(" ")),
            ])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::AuthFailed(format!("Démarrage device flow échoué : {e}")))?;

        Ok(resp.json::<DeviceCodeResponse>().await?)
    }

    /// Étape 2 : effectue le polling jusqu'à ce que l'utilisateur ait validé
    /// le code dans son navigateur, ou jusqu'à expiration.
    ///
    /// `on_tick` permet de notifier le frontend à chaque tentative (pour un
    /// éventuel indicateur "en attente d'autorisation...").
    pub async fn poll_for_tokens(
        &self,
        device: &DeviceCodeResponse,
        on_tick: impl Fn(),
    ) -> AppResult<TwitchTokens> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(device.expires_in);
        let mut interval = Duration::from_secs(device.interval.max(1));

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(AppError::AuthFailed(
                    "Le code d'autorisation a expiré, veuillez réessayer".into(),
                ));
            }
            tokio::time::sleep(interval).await;
            on_tick();

            let resp = self
                .http
                .post(format!("{TWITCH_OAUTH_BASE}/token"))
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("device_code", device.device_code.as_str()),
                    (
                        "grant_type",
                        "urn:ietf:params:oauth:grant-type:device_code",
                    ),
                ])
                .send()
                .await?;

            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();

            if status.is_success() {
                let access_token = body["access_token"]
                    .as_str()
                    .ok_or_else(|| AppError::AuthFailed("Réponse Twitch inattendue".into()))?
                    .to_string();
                let refresh_token = body["refresh_token"].as_str().unwrap_or_default().to_string();
                let expires_in = body["expires_in"].as_i64().unwrap_or(14400);

                let (login, user_id) = self.fetch_user_identity(&access_token).await?;

                return Ok(TwitchTokens {
                    access_token,
                    refresh_token,
                    expires_at_unix: chrono::Utc::now().timestamp() + expires_in,
                    login,
                    user_id,
                });
            }

            // "authorization_pending" -> on continue de sonder.
            // "slow_down" -> Twitch demande d'espacer les requêtes.
            match body["message"].as_str().unwrap_or_default() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += Duration::from_secs(5);
                    continue;
                }
                other => {
                    return Err(AppError::AuthFailed(format!(
                        "Autorisation refusée ou invalide : {other}"
                    )))
                }
            }
        }
    }

    /// Rafraîchit un token expiré. Note : la documentation Twitch indique
    /// que `client_secret` n'est théoriquement pas requis pour un client
    /// public, mais certains comptes d'application peuvent l'exiger selon
    /// leur configuration — dans ce cas la requête échoue explicitement et
    /// l'application retombe sur un nouveau `start_device_flow`.
    pub async fn refresh(&self, refresh_token: &str) -> AppResult<TwitchTokens> {
        let resp = self
            .http
            .post(format!("{TWITCH_OAUTH_BASE}/token"))
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::AuthRequired);
        }

        let body: serde_json::Value = resp.json().await?;
        let access_token = body["access_token"]
            .as_str()
            .ok_or(AppError::AuthRequired)?
            .to_string();
        let refresh_token = body["refresh_token"]
            .as_str()
            .unwrap_or(refresh_token)
            .to_string();
        let expires_in = body["expires_in"].as_i64().unwrap_or(14400);

        let (login, user_id) = self.fetch_user_identity(&access_token).await?;

        Ok(TwitchTokens {
            access_token,
            refresh_token,
            expires_at_unix: chrono::Utc::now().timestamp() + expires_in,
            login,
            user_id,
        })
    }

    async fn fetch_user_identity(&self, access_token: &str) -> AppResult<(String, String)> {
        let resp = self
            .http
            .get("https://api.twitch.tv/helix/users")
            .bearer_auth(access_token)
            .header("Client-Id", &self.client_id)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::AuthFailed(format!("Impossible de résoudre l'identité : {e}")))?;

        let body: serde_json::Value = resp.json().await?;
        let user = body["data"]
            .get(0)
            .ok_or_else(|| AppError::AuthFailed("Aucun utilisateur retourné par Helix".into()))?;

        Ok((
            user["login"].as_str().unwrap_or_default().to_string(),
            user["id"].as_str().unwrap_or_default().to_string(),
        ))
    }
}
