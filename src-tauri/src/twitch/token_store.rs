//! Stockage sécurisé des jetons d'authentification Twitch.
//!
//! ## Pourquoi le trousseau système plutôt que le fichier `config.json` ?
//!
//! `config.json` est un fichier en clair, exportable/importable par
//! l'utilisateur (fonctionnalité demandée du cahier des charges). Y stocker
//! un `access_token`/`refresh_token` exposerait ces secrets à toute
//! personne ayant accès au fichier exporté (ex: partagé par erreur sur
//! Discord pour du support). Les jetons sont donc stockés séparément via la
//! crate `keyring`, qui utilise :
//! - **Windows** : Credential Manager
//! - **macOS** : Keychain
//! - **Linux** : Secret Service (GNOME Keyring / KWallet via D-Bus)
//!
//! Ainsi, exporter/importer la configuration ne déplace jamais de secret :
//! après un import sur une nouvelle machine, l'utilisateur doit simplement
//! se réauthentifier (Device Code Flow), ce qui est le comportement attendu
//! pour un produit distribué publiquement.

use crate::error::{AppError, AppResult};
use crate::twitch::auth::TwitchTokens;

const SERVICE_NAME: &str = "twitch-voice-reader";

pub struct TokenStore;

impl TokenStore {
    fn entry(account: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, account)
            .map_err(|e| AppError::Internal(format!("Trousseau système inaccessible : {e}")))
    }

    pub fn save(tokens: &TwitchTokens) -> AppResult<()> {
        let json = serde_json::to_string(tokens)?;
        Self::entry(&tokens.login)?
            .set_password(&json)
            .map_err(|e| AppError::Internal(format!("Échec d'écriture dans le trousseau : {e}")))?;
        // On mémorise également le dernier compte utilisé pour pouvoir le
        // retrouver au démarrage sans connaître le login à l'avance.
        Self::entry("__last_account__")?
            .set_password(&tokens.login)
            .ok();
        Ok(())
    }

    pub fn load_last() -> Option<TwitchTokens> {
        let last_login = Self::entry("__last_account__").ok()?.get_password().ok()?;
        let json = Self::entry(&last_login).ok()?.get_password().ok()?;
        serde_json::from_str(&json).ok()
    }

    pub fn clear(login: &str) -> AppResult<()> {
        if let Ok(entry) = Self::entry(login) {
            let _ = entry.delete_credential();
        }
        Ok(())
    }
}
