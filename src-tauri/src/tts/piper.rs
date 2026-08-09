//! Intégration de [Piper](https://github.com/rhasspy/piper), moteur TTS
//! neuronal local, fonctionnant entièrement hors ligne une fois les modèles
//! de voix `.onnx` téléchargés.
//!
//! ## Stratégie d'invocation : processus à usage unique vs serveur persistant
//!
//! | Approche | Latence | Robustesse | Complexité |
//! |---|---|---|---|
//! | **Spawn d'un process Piper par message (retenu pour le MVP)** | ~50-150ms de démarrage process supplémentaires | Un crash Piper n'affecte qu'un message | Faible |
//! | Process Piper persistant, texte via stdin en continu | Latence minimale (pas de redémarrage) | Un crash coupe toute la synthèse en cours | Élevée (framing des réponses, gestion des erreurs partielles) |
//!
//! Le MVP spawn un processus par message avec sortie PCM brute directement
//! sur stdout (`--output_raw`), ce qui évite tout fichier temporaire sur
//! disque. La file d'attente ([`super::queue`]) lisse de toute façon le
//! débit à quelques messages par minute au maximum (cf. anti-spam), rendant
//! le coût de démarrage du process négligeable en pratique. Le passage à un
//! process persistant est documenté comme optimisation Phase 3 si la
//! latence perçue devient un problème en usage réel.
//!
//! ## Piège corrigé : fuite de processus (`tokio::process::Child`)
//!
//! `tokio::process::Child` ne tue **pas** son processus enfant lors du
//! `Drop` (contrairement à ce qu'on pourrait intuitivement attendre — c'est
//! documenté mais facile à manquer). Concrètement : si `synthesize()` sort
//! en erreur avant `child.wait()`, ou si l'appelant annule la future (ex:
//! le `tokio::time::timeout` de la file TTS, voir `tts/queue.rs`), le
//! process Piper sous-jacent devient orphelin et continue de tourner
//! indéfiniment en arrière-plan. `KillOnDropChild` ci-dessous corrige ce
//! comportement en enveloppant le `Child` dans un garde qui envoie un
//! signal de terminaison au `Drop`, quel que soit le chemin de sortie
//! (retour normal, `?`, ou annulation externe).

use crate::error::{AppError, AppResult};
use crate::tts::installer::{self, InstallProgress};
use parking_lot::RwLock;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

/// Fréquence d'échantillonnage standard des voix Piper (mono, 16 bits).
pub const PIPER_SAMPLE_RATE: u32 = 22_050;

pub struct PiperEngine {
    /// Chemin résolu du binaire. `RwLock` plutôt qu'un simple `PathBuf` :
    /// mis à jour dynamiquement après une installation automatique
    /// réussie, potentiellement bien après la construction de `PiperEngine`
    /// (celle-ci a lieu avant qu'on sache si Piper est déjà présent).
    binary_path: RwLock<Option<PathBuf>>,
    /// Sérialise les appels concurrents à `ensure_ready()` : sans ce
    /// verrou, un appel automatique au démarrage et un appel explicite
    /// déclenché par l'ouverture de l'onglet "Voix et TTS" pourraient
    /// tous deux constater `binary_path == None` et lancer chacun un
    /// téléchargement vers le même dossier simultanément.
    install_lock: tokio::sync::Mutex<()>,
    /// Dossier où l'installation automatique télécharge Piper si aucun
    /// binaire n'est trouvé ailleurs (ressources bundlées, chemin
    /// explicite en configuration). Distinct de `voices_dir` : le binaire
    /// et les voix ont des cycles de vie indépendants (un binaire déjà
    /// bundlé n'implique pas que les voix le soient aussi, et inversement).
    auto_install_dir: PathBuf,
    voices_dir: PathBuf,
}

/// Résultat brut d'une synthèse : échantillons PCM 16 bits signés, mono.
pub struct SynthesizedAudio {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}

/// Garde RAII autour d'un `Child` : envoie un signal de terminaison au
/// `Drop` tant que le process n'a pas été proprement attendu via
/// `wait_and_disarm()`. Voir la note de module ci-dessus.
struct KillOnDropChild {
    child: Child,
    /// Mis à `true` une fois `child.wait()` appelé avec succès : à ce
    /// stade le process est déjà terminé, inutile (et légèrement coûteux)
    /// d'émettre un signal de terminaison au `Drop`.
    waited: bool,
}

impl KillOnDropChild {
    fn new(child: Child) -> Self {
        Self { child, waited: false }
    }

    /// Attend la fin du process. Marque le garde comme "désarmé" en cas de
    /// succès : le process s'est terminé de lui-même, rien à tuer au `Drop`.
    async fn wait_and_disarm(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let status = self.child.wait().await?;
        self.waited = true;
        Ok(status)
    }
}

impl Deref for KillOnDropChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl DerefMut for KillOnDropChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

impl Drop for KillOnDropChild {
    fn drop(&mut self) {
        if !self.waited {
            // `start_kill()` est synchrone et non bloquant (on ne peut pas
            // `.await` dans un `Drop`) : il suffit à garantir qu'aucun
            // process Piper ne survit à sa future si celle-ci est abandonnée
            // en cours de route (erreur précoce ou annulation par timeout).
            let _ = self.child.start_kill();
        }
    }
}

impl PiperEngine {
    /// `binary_path` : chemin déjà connu au démarrage (ressources bundlées
    /// ou chemin explicite en configuration), s'il a pu être résolu —
    /// `None` sinon, auquel cas `ensure_ready()` tentera une installation
    /// automatique dans `auto_install_dir` au moment voulu plutôt qu'au
    /// démarrage (ne bloque jamais le lancement de l'application).
    pub fn new(binary_path: Option<PathBuf>, auto_install_dir: PathBuf, voices_dir: PathBuf) -> Self {
        Self {
            binary_path: RwLock::new(binary_path),
            install_lock: tokio::sync::Mutex::new(()),
            auto_install_dir,
            voices_dir,
        }
    }

    fn resolved_binary_path(&self) -> Option<PathBuf> {
        self.binary_path.read().clone()
    }

    /// Vérifie que le binaire Piper est exécutable et retourne sa version
    /// (utilisé par l'écran de diagnostic "Voix et TTS"). Purement
    /// diagnostique : ne tente aucune installation, contrairement à
    /// `ensure_ready()`.
    pub async fn check_installation(&self) -> AppResult<String> {
        let Some(binary_path) = self.resolved_binary_path() else {
            return Err(AppError::TtsUnavailable(
                "Piper n'est pas encore installé sur cette machine.".into(),
            ));
        };
        let output = Command::new(&binary_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| AppError::TtsUnavailable(format!("Binaire Piper introuvable : {e}")))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// S'assure que Piper (binaire + voix par défaut) est prêt à l'emploi,
    /// en déclenchant une installation automatique si nécessaire. Idempotent
    /// et sûr à appeler à répétition (ex: à chaque ouverture de l'onglet
    /// "Voix et TTS", ou juste avant chaque synthèse — voir `synthesize()`).
    pub async fn ensure_ready(&self, on_progress: impl Fn(InstallProgress) + Send + Sync + 'static) -> AppResult<()> {
        let _guard = self.install_lock.lock().await;

        if self.resolved_binary_path().is_none() {
            let installed_path = installer::ensure_piper_binary(&self.auto_install_dir, &on_progress).await?;
            *self.binary_path.write() = Some(installed_path);
        }
        // `Done` n'est émis qu'ici, une fois le binaire *et* la voix par
        // défaut confirmés prêts — jamais depuis `installer::ensure_piper_binary`
        // seul, qui ne couvre qu'une étape intermédiaire. Émettre "Done"
        // trop tôt ferait passer l'interface à "prêt" alors que le
        // téléchargement de la voix pourrait encore être en cours.
        installer::ensure_default_voice(&self.voices_dir, &on_progress).await?;
        on_progress(InstallProgress::Done);
        Ok(())
    }

    pub fn voice_model_path(&self, voice_id: &str) -> PathBuf {
        self.voices_dir.join(format!("{voice_id}.onnx"))
    }

    pub fn is_voice_installed(&self, voice_id: &str) -> bool {
        self.voice_model_path(voice_id).exists()
    }

    /// Synthétise `text` avec la voix `voice_id` et retourne les échantillons
    /// PCM bruts (aucune écriture disque). `length_scale` contrôle la
    /// vitesse d'élocution côté Piper (1.0 = normal, >1.0 = plus lent).
    pub async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        length_scale: f32,
    ) -> AppResult<SynthesizedAudio> {
        // Filet de sécurité : si l'installation automatique n'a encore
        // jamais été tentée (ex: l'utilisateur a directement cliqué
        // "Tester" sans jamais ouvrir l'onglet Voix et TTS auparavant dans
        // cette session), on la déclenche ici plutôt que d'échouer. Sans
        // callback de progression détaillé : cet appel silencieux est un
        // filet de sécurité, pas le chemin principal (voir
        // `commands::tts::tts_ensure_installed`, appelé explicitement par
        // le frontend avec suivi de progression visible).
        if self.resolved_binary_path().is_none() {
            self.ensure_ready(|_| {}).await?;
        }

        let binary_path = self.resolved_binary_path().ok_or_else(|| {
            AppError::TtsUnavailable("Le moteur Piper n'a pas pu être installé automatiquement.".into())
        })?;

        let model_path = self.voice_model_path(voice_id);
        if !model_path.exists() {
            return Err(AppError::TtsUnavailable(format!(
                "Modèle de voix introuvable : {voice_id} (attendu : {}). \
                 Ouvrez l'onglet « Voix et TTS » pour déclencher son téléchargement automatique.",
                model_path.display()
            )));
        }

        let child = Command::new(&binary_path)
            .arg("--model")
            .arg(&model_path)
            .arg("--output_raw")
            .arg("--length_scale")
            .arg(length_scale.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::TtsUnavailable(format!("Impossible de démarrer Piper : {e}")))?;

        // À partir d'ici le process est réellement démarré : on le
        // protège immédiatement par le garde kill-on-drop, avant toute
        // opération faillible.
        let mut guarded = KillOnDropChild::new(child);

        // Piper traite l'entrée standard ligne par ligne : un saut de
        // ligne au milieu du texte serait interprété comme deux énoncés
        // distincts. On neutralise ce cas de bord plutôt que de laisser
        // Piper produire un résultat tronqué ou inattendu.
        let sanitized_text = text.replace(['\n', '\r'], " ");

        let io_result = Self::write_stdin_and_read_stdout(&mut guarded, &sanitized_text).await;

        let raw_pcm = match io_result {
            Ok(pcm) => pcm,
            Err(e) => return Err(e), // `guarded` est droppé ici -> kill garanti
        };

        let status = guarded
            .wait_and_disarm()
            .await
            .map_err(|e| AppError::TtsUnavailable(format!("Piper : échec en attendant la fin du process : {e}")))?;

        if !status.success() {
            let mut stderr_buf = Vec::new();
            if let Some(mut stderr) = guarded.stderr.take() {
                stderr.read_to_end(&mut stderr_buf).await.ok();
            }
            return Err(AppError::TtsUnavailable(format!(
                "Piper a retourné une erreur (code {:?}) : {}",
                status.code(),
                String::from_utf8_lossy(&stderr_buf)
            )));
        }

        // Le flux brut est du PCM 16 bits little-endian mono.
        let samples: Vec<i16> = raw_pcm
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();

        if samples.is_empty() {
            // Piper a rendu la main avec succès mais n'a produit aucun
            // échantillon (ex: modèle incompatible avec la version du
            // binaire, texte vide après nettoyage). Sans ce contrôle,
            // l'appelant recevrait un `Ok` silencieux et ne comprendrait
            // jamais pourquoi "rien ne se passe" — symptôme observé sur le
            // bouton de test vocal.
            return Err(AppError::TtsUnavailable(
                "Piper n'a produit aucun échantillon audio (sortie vide). Vérifiez la compatibilité \
                 entre la version du binaire Piper et le modèle de voix utilisé."
                    .into(),
            ));
        }

        Ok(SynthesizedAudio {
            samples,
            sample_rate: PIPER_SAMPLE_RATE,
        })
    }

    async fn write_stdin_and_read_stdout(child: &mut Child, text: &str) -> AppResult<Vec<u8>> {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.shutdown().await?;
        }

        let mut raw_pcm = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_end(&mut raw_pcm).await?;
        }
        Ok(raw_pcm)
    }

    /// Liste les voix installées localement (fichiers `.onnx` présents dans
    /// le dossier des voix), utilisée par le sélecteur de voix du frontend.
    pub fn list_installed_voices(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.voices_dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("onnx") {
                    path.file_stem().and_then(|s| s.to_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect()
    }
}
