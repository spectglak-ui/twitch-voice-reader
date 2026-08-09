//! Installation automatique du moteur Piper et de la voix par défaut.
//!
//! ## Pourquoi ce module existe
//!
//! Historiquement, l'installation de Piper reposait sur un script externe
//! (`scripts/install-piper.sh`/`.ps1`) que l'utilisateur devait exécuter
//! manuellement *avant* le premier lancement. En pratique, cette étape a
//! plusieurs points de défaillance indépendants de la logique même du
//! téléchargement : politique d'exécution PowerShell qui bloque les
//! scripts non signés par défaut sur beaucoup de configurations Windows,
//! oubli pur et simple de l'étape, ou exécution depuis le mauvais
//! répertoire. Le symptôme observé (`resources/piper/` vide, aucune erreur
//! nulle part) correspond exactement à un script qui n'a jamais pu
//! s'exécuter — sans que rien ne le signale clairement.
//!
//! Ce module déplace la responsabilité du téléchargement **dans
//! l'application elle-même** : au démarrage, puis à la demande (bouton
//! "Réessayer" si le premier essai échoue faute de réseau), l'application
//! vérifie la présence de Piper et, si absent, le télécharge et l'installe
//! automatiquement dans le répertoire de données utilisateur — sans
//! dépendre d'une étape manuelle ni de PowerShell.
//!
//! Le script externe reste utile pour le **packaging** (bundler Piper dans
//! l'installeur final afin qu'un utilisateur final n'ait pas besoin d'accès
//! réseau au premier lancement — voir `docs/GUIDE_COMPILATION.md`), mais
//! n'est plus un prérequis pour que l'application fonctionne en développement.
//!
//! ## Risque connu à moyen terme : dépôt archivé
//!
//! Le dépôt source de Piper (`rhasspy/piper`) a été **archivé** par son
//! propriétaire (lecture seule depuis octobre 2025) ; le développement se
//! poursuit sous [`OHF-Voice/piper1-gpl`](https://github.com/OHF-Voice/piper1-gpl),
//! qui ne publie plus d'exécutable Windows autonome (distribution via
//! `pip install piper-tts`, nécessitant un interpréteur Python). Les
//! anciens artefacts (`2023.11.14-2`) restent téléchargeables — l'archivage
//! d'un dépôt GitHub ne supprime pas ses releases publiées — mais ne
//! recevront plus jamais de mise à jour. Ce module cible donc
//! délibérément cette dernière version figée. Une migration vers
//! `piper1-gpl` (packaging Python embarqué) est documentée comme piste de
//! Phase 4 dans le cahier technique si cette dépendance venait à devenir
//! un problème (ex: URLs qui cessent de répondre).

use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Dernière version publiée avant l'archivage du dépôt source.
const PIPER_VERSION: &str = "2023.11.14-2";

/// Taille minimale plausible d'une archive Piper valide. Sert à détecter
/// immédiatement un téléchargement qui aurait silencieusement récupéré une
/// page d'erreur HTML (souvent quelques centaines d'octets) plutôt que le
/// binaire réel (plusieurs dizaines de Mo), sans attendre l'échec de
/// l'extraction pour s'en apercevoir.
const MIN_PLAUSIBLE_ARCHIVE_BYTES: u64 = 5 * 1024 * 1024; // 5 Mo

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "stage")]
pub enum InstallProgress {
    CheckingExisting,
    Downloading { label: String, percent: Option<u8> },
    Extracting,
    Verifying,
    DownloadingVoice { label: String },
    Done,
    Error { message: String },
}

fn asset_name_for_platform() -> AppResult<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("piper_windows_amd64.zip"),
        ("linux", "x86_64") => Ok("piper_linux_x86_64.tar.gz"),
        ("linux", "aarch64") => Ok("piper_linux_aarch64.tar.gz"),
        ("linux", "arm") => Ok("piper_linux_armv7l.tar.gz"),
        ("macos", "x86_64") => Ok("piper_macos_x64.tar.gz"),
        ("macos", "aarch64") => Ok("piper_macos_aarch64.tar.gz"),
        (os, arch) => Err(AppError::TtsUnavailable(format!(
            "Aucun binaire Piper pré-compilé n'est publié pour cette plateforme \
             ({os}/{arch}). Installez Piper manuellement et renseignez son chemin \
             dans les paramètres avancés."
        ))),
    }
}

/// Vérifie si le binaire Piper est déjà présent à l'emplacement géré par
/// l'application (`install_dir`), sans déclencher de téléchargement.
pub fn find_installed_binary(install_dir: &Path) -> Option<PathBuf> {
    let binary_name = if cfg!(target_os = "windows") { "piper.exe" } else { "piper" };
    let direct = install_dir.join(binary_name);
    if direct.is_file() {
        return Some(direct);
    }
    // Recherche défensive sur deux niveaux de profondeur, au cas où
    // l'archive contiendrait un dossier imbriqué (non observé dans la
    // structure documentée des releases Piper au moment de l'écriture,
    // mais peu coûteux à couvrir et évite une régression silencieuse si
    // cette structure changeait sur une future release).
    find_in_subdirs(install_dir, binary_name, 2)
}

fn find_in_subdirs(dir: &Path, filename: &str, max_depth: u8) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join(filename);
            if candidate.is_file() {
                return Some(candidate);
            }
            if let Some(found) = find_in_subdirs(&path, filename, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

/// Télécharge et installe Piper dans `install_dir` si absent. Retourne le
/// chemin du binaire prêt à l'emploi. Idempotent : ne retélécharge rien si
/// déjà présent.
pub async fn ensure_piper_binary(
    install_dir: &Path,
    on_progress: impl Fn(InstallProgress),
) -> AppResult<PathBuf> {
    on_progress(InstallProgress::CheckingExisting);
    if let Some(existing) = find_installed_binary(install_dir) {
        return Ok(existing);
    }

    std::fs::create_dir_all(install_dir)?;

    let asset = asset_name_for_platform()?;
    let url = format!("https://github.com/rhasspy/piper/releases/download/{PIPER_VERSION}/{asset}");

    let archive_bytes = download_with_progress(&url, asset, &on_progress).await?;

    if (archive_bytes.len() as u64) < MIN_PLAUSIBLE_ARCHIVE_BYTES {
        return Err(AppError::TtsUnavailable(format!(
            "Le fichier téléchargé ({} octets) est trop petit pour être une archive Piper \
             valide — probablement une page d'erreur renvoyée par le serveur plutôt que le \
             binaire. URL tentée : {url}",
            archive_bytes.len()
        )));
    }

    on_progress(InstallProgress::Extracting);
    extract_archive(asset, &archive_bytes, install_dir)?;

    on_progress(InstallProgress::Verifying);
    let binary_path = find_installed_binary(install_dir).ok_or_else(|| {
        AppError::TtsUnavailable(
            "L'archive Piper a été téléchargée et extraite, mais aucun exécutable n'a été \
             retrouvé à l'intérieur. La structure de l'archive a peut-être changé côté \
             éditeur."
                .into(),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&binary_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(perms.mode() | 0o755);
            std::fs::set_permissions(&binary_path, perms).ok();
        }
    }

    verify_binary_runs(&binary_path).await?;

    Ok(binary_path)
}

/// Télécharge la voix française par défaut (modèle + config JSON associée)
/// si absente. Hébergée sur Hugging Face (dépôt communautaire des voix
/// Piper), indépendamment du binaire lui-même.
pub async fn ensure_default_voice(voices_dir: &Path, on_progress: impl Fn(InstallProgress)) -> AppResult<()> {
    const VOICE_ID: &str = "fr_FR-siwis-medium";
    const BASE_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium";

    let model_path = voices_dir.join(format!("{VOICE_ID}.onnx"));
    let config_path = voices_dir.join(format!("{VOICE_ID}.onnx.json"));
    if model_path.is_file() && config_path.is_file() {
        return Ok(());
    }

    std::fs::create_dir_all(voices_dir)?;
    on_progress(InstallProgress::DownloadingVoice {
        label: VOICE_ID.to_string(),
    });

    let model_bytes = download_with_progress(&format!("{BASE_URL}/{VOICE_ID}.onnx"), VOICE_ID, &on_progress).await?;
    if (model_bytes.len() as u64) < 1024 * 1024 {
        // Un modèle .onnx valide fait plusieurs dizaines de Mo ; quelques
        // Ko indiquent presque certainement une réponse d'erreur.
        return Err(AppError::TtsUnavailable(format!(
            "Le modèle de voix téléchargé ({} octets) est anormalement petit.",
            model_bytes.len()
        )));
    }
    let config_bytes = reqwest::get(format!("{BASE_URL}/{VOICE_ID}.onnx.json"))
        .await?
        .error_for_status()
        .map_err(|e| AppError::TtsUnavailable(format!("Téléchargement de la config de voix échoué : {e}")))?
        .bytes()
        .await?;

    std::fs::write(&model_path, &model_bytes)?;
    std::fs::write(&config_path, &config_bytes)?;
    Ok(())
}

async fn download_with_progress(
    url: &str,
    label: &str,
    on_progress: &impl Fn(InstallProgress),
) -> AppResult<Vec<u8>> {
    let response = reqwest::get(url)
        .await?
        .error_for_status()
        .map_err(|e| AppError::TtsUnavailable(format!("Téléchargement échoué ({label}) : {e}")))?;

    let total_size = response.content_length();
    let mut downloaded: u64 = 0;
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    let mut last_reported_percent: i32 = -1;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::TtsUnavailable(format!("Flux de téléchargement interrompu : {e}")))?;
        downloaded += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);

        if let Some(total) = total_size {
            let percent = ((downloaded as f64 / total as f64) * 100.0).round() as i32;
            if percent != last_reported_percent {
                last_reported_percent = percent;
                on_progress(InstallProgress::Downloading {
                    label: label.to_string(),
                    percent: Some(percent.clamp(0, 100) as u8),
                });
            }
        } else {
            on_progress(InstallProgress::Downloading {
                label: label.to_string(),
                percent: None,
            });
        }
    }

    Ok(bytes)
}

fn extract_archive(asset_name: &str, archive_bytes: &[u8], destination: &Path) -> AppResult<()> {
    if asset_name.ends_with(".zip") {
        let cursor = std::io::Cursor::new(archive_bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| AppError::TtsUnavailable(format!("Archive Piper invalide (.zip) : {e}")))?;
        archive
            .extract(destination)
            .map_err(|e| AppError::TtsUnavailable(format!("Échec d'extraction de l'archive .zip : {e}")))?;
    } else if asset_name.ends_with(".tar.gz") {
        let decoder = flate2::read::GzDecoder::new(archive_bytes);
        let mut tar_archive = tar::Archive::new(decoder);
        tar_archive
            .unpack(destination)
            .map_err(|e| AppError::TtsUnavailable(format!("Échec d'extraction de l'archive .tar.gz : {e}")))?;
    } else {
        return Err(AppError::TtsUnavailable(format!(
            "Format d'archive non pris en charge : {asset_name}"
        )));
    }
    Ok(())
}

/// Dernier contrôle avant de considérer l'installation réussie : exécute
/// réellement le binaire avec `--version` plutôt que de se fier uniquement
/// à sa présence sur disque (un exécutable présent mais incompatible avec
/// l'architecture système, ou corrompu, échouerait silencieusement plus
/// tard au premier vrai essai de synthèse — ce contrôle le détecte
/// immédiatement, avec un message d'erreur exploitable).
async fn verify_binary_runs(binary_path: &Path) -> AppResult<()> {
    let output = tokio::process::Command::new(binary_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            AppError::TtsUnavailable(format!(
                "Le binaire Piper téléchargé n'a pas pu être exécuté ({e}). Il est peut-être \
                 incompatible avec votre système, ou bloqué par un antivirus."
            ))
        })?;

    if !output.status.success() {
        return Err(AppError::TtsUnavailable(format!(
            "Le binaire Piper téléchargé s'est exécuté mais a retourné une erreur : {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}
