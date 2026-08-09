//! Persistance disque de [`AppConfig`].
//!
//! Stratégie : écriture atomique (fichier temporaire + rename) pour éviter
//! toute corruption si l'application est fermée pendant une sauvegarde, et
//! sauvegarde d'une copie `.bak` avant chaque écriture.

use super::schema::AppConfig;
use crate::error::{AppError, AppResult};
use parking_lot::RwLock;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Enveloppe thread-safe autour de la configuration en mémoire.
/// Partagée via `tauri::State` entre toutes les commandes.
#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    inner: Arc<RwLock<AppConfig>>,
}

impl ConfigStore {
    /// Charge la configuration depuis `app_data_dir/config.json`, ou crée
    /// une configuration par défaut si le fichier n'existe pas encore.
    pub fn load(app_data_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(app_data_dir)?;
        let path = app_data_dir.join("config.json");

        let config = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_else(|e| {
                tracing::warn!("config.json corrompu ({e}), rechargement des valeurs par défaut");
                AppConfig::default()
            })
        } else {
            AppConfig::default()
        };

        let store = Self {
            path,
            inner: Arc::new(RwLock::new(config)),
        };
        store.save()?; // garantit la présence d'un fichier valide dès le premier lancement
        Ok(store)
    }

    pub fn get(&self) -> AppConfig {
        self.inner.read().clone()
    }

    /// Remplace intégralement la configuration et persiste immédiatement.
    pub fn set(&self, config: AppConfig) -> AppResult<()> {
        *self.inner.write() = config;
        self.save()
    }

    /// Applique une mutation ciblée puis persiste (évite les races entre
    /// lecture et écriture partielle depuis le frontend).
    pub fn update<F>(&self, mutator: F) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig),
    {
        {
            let mut guard = self.inner.write();
            mutator(&mut guard);
        }
        self.save()?;
        Ok(self.get())
    }

    fn save(&self) -> AppResult<()> {
        let config = self.inner.read();
        let json = serde_json::to_string_pretty(&*config)?;

        if self.path.exists() {
            let backup_path = self.path.with_extension("json.bak");
            let _ = fs::copy(&self.path, backup_path);
        }

        let tmp_path = self.path.with_extension("json.tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Exporte la configuration courante vers un chemin arbitraire choisi
    /// par l'utilisateur (dialogue "Enregistrer sous").
    pub fn export_to(&self, destination: &Path) -> AppResult<()> {
        let config = self.inner.read();
        let json = serde_json::to_string_pretty(&*config)?;
        fs::write(destination, json)?;
        Ok(())
    }

    /// Importe une configuration depuis un fichier JSON externe, la valide
    /// puis la persiste comme configuration active.
    pub fn import_from(&self, source: &Path) -> AppResult<AppConfig> {
        let raw = fs::read_to_string(source)?;
        let imported: AppConfig = serde_json::from_str(&raw)
            .map_err(|e| AppError::InvalidConfig(format!("JSON invalide : {e}")))?;
        self.set(imported.clone())?;
        Ok(imported)
    }

    pub fn reset_to_defaults(&self) -> AppResult<AppConfig> {
        self.set(AppConfig::default())?;
        Ok(self.get())
    }
}
