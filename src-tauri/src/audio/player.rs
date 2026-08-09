//! Lecture audio des échantillons PCM produits par Piper.
//!
//! ## Choix d'architecture important : thread dédié plutôt que `Send` direct
//!
//! `rodio::OutputStream` (et le `cpal::Stream` sous-jacent) n'est **pas**
//! `Send` de manière fiable sur toutes les plateformes/backends (le handle
//! natif du flux audio n'est pas garanti thread-safe par les API système
//! sous-jacentes). Or Tauri exige que tout état managé (`app.manage(...)`)
//! soit `Send + Sync + 'static`. Stocker directement un `OutputStream` dans
//! `AppState` — même derrière un `Arc<Mutex<...>>` — ne compilerait donc pas
//! de façon fiable sur toutes les cibles.
//!
//! **Solution retenue** : le flux de sortie audio est créé et possédé
//! entièrement par un **thread OS dédié** qui ne le laisse jamais
//! s'échapper. Le reste de l'application communique avec ce thread via un
//! canal de commandes (`std::sync::mpsc`), et `AudioPlayer` — la structure
//! exposée au reste du code — ne contient plus qu'un `Sender`, trivialement
//! `Send + Sync`. C'est le pattern standard pour intégrer `rodio`/`cpal`
//! dans une application async multi-thread (voir aussi la documentation de
//! `cpal` sur le sujet).
//!
//! ## Limitation connue : ajustement de la hauteur de voix (pitch)
//!
//! Le pitch est simulé en rejouant le buffer à une fréquence
//! d'échantillonnage déclarée différente de la fréquence native (technique
//! "vinyle"), ce qui modifie hauteur **et** vitesse simultanément. Un vrai
//! pitch-shift indépendant du tempo nécessiterait un phase-vocoder ou un
//! resampling asynchrone de qualité (crate `rubato`) — amélioration prévue
//! en Phase 3, documentée aussi dans le cahier technique.

use crate::error::{AppError, AppResult};
use rodio::{OutputStream, Sink, Source};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

struct PlaybackRequest {
    samples: Vec<i16>,
    sample_rate: u32,
    volume: f32,
    pitch: f32,
    /// Signalé une fois la lecture terminée (ou en erreur), pour que
    /// l'appelant async puisse `.await` la fin de la lecture sans bloquer
    /// le runtime Tokio.
    done: tokio::sync::oneshot::Sender<AppResult<()>>,
}

enum AudioCommand {
    Play(PlaybackRequest),
    SwitchDevice {
        device_name: Option<String>,
        ack: tokio::sync::oneshot::Sender<AppResult<()>>,
    },
}

pub struct AudioPlayer {
    command_tx: std_mpsc::Sender<AudioCommand>,
}

/// Source rodio adaptée à partir d'un buffer PCM 16 bits mono déjà en mémoire.
struct RawPcmSource {
    samples: std::vec::IntoIter<i16>,
    sample_rate: u32,
}

impl Iterator for RawPcmSource {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        self.samples.next()
    }
}

impl Source for RawPcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// État interne du thread audio : ouvert sur un périphérique donné, recréé
/// entièrement lors d'un changement de périphérique.
struct AudioThreadState {
    _stream: OutputStream,
    sink: Sink,
}

impl AudioThreadState {
    fn open(device_name: Option<&str>) -> AppResult<Self> {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();
        let device = match device_name {
            Some(name) => host
                .output_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().map(|n| n == name).unwrap_or(false)))
                .ok_or_else(|| AppError::AudioDevice(format!("Périphérique introuvable : {name}")))?,
            None => host
                .default_output_device()
                .ok_or_else(|| AppError::AudioDevice("Aucun périphérique de sortie par défaut".into()))?,
        };

        let (stream, stream_handle) = OutputStream::try_from_device(&device)
            .map_err(|e| AppError::AudioDevice(format!("Impossible d'ouvrir le flux audio : {e}")))?;
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| AppError::AudioDevice(format!("Impossible de créer le sink audio : {e}")))?;

        Ok(Self { _stream: stream, sink })
    }
}

impl AudioPlayer {
    /// Démarre le thread audio dédié et retourne un handle léger pour lui
    /// envoyer des commandes. `device_name` = `None` pour le périphérique
    /// par défaut du système.
    ///
    /// Important : le flux de sortie (`AudioThreadState::open`) est créé
    /// **à l'intérieur** du thread fraîchement spawné, jamais avant. S'il
    /// était créé sur le thread appelant puis déplacé dans la closure du
    /// thread, on retomberait dans le problème `!Send` que cette
    /// architecture cherche justement à éviter (voir doc de module).
    /// On utilise un rendez-vous bloquant (`std::sync::mpsc`) pour tout de
    /// même remonter une erreur d'ouverture de périphérique de façon
    /// synchrone à l'appelant.
    pub fn new(device_name: Option<&str>) -> AppResult<Self> {
        let (command_tx, command_rx) = std_mpsc::channel::<AudioCommand>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<AppResult<()>>();
        let device_name = device_name.map(|s| s.to_string());

        std::thread::Builder::new()
            .name("twitch-voice-reader-audio".into())
            .spawn(move || match AudioThreadState::open(device_name.as_deref()) {
                Ok(state) => {
                    ready_tx.send(Ok(())).ok();
                    audio_thread_loop(state, command_rx);
                }
                Err(e) => {
                    ready_tx.send(Err(e)).ok();
                    // Le thread s'arrête ici : les futurs envois sur
                    // `command_tx` échoueront proprement (canal fermé),
                    // remontés comme `AppError::AudioDevice` par les
                    // méthodes publiques ci-dessous.
                }
            })
            .map_err(|e| AppError::Internal(format!("Impossible de démarrer le thread audio : {e}")))?;

        ready_rx
            .recv()
            .map_err(|_| AppError::AudioDevice("Le thread audio s'est arrêté avant initialisation".into()))??;

        Ok(Self { command_tx })
    }

    /// Joue un buffer PCM et attend la fin de la lecture (la file TTS
    /// dépend de ce comportement pour garantir la séquentialité).
    pub async fn play_pcm(
        &self,
        samples: Vec<i16>,
        sample_rate: u32,
        volume: f32,
        pitch: f32,
    ) -> AppResult<()> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(AudioCommand::Play(PlaybackRequest {
                samples,
                sample_rate,
                volume,
                pitch,
                done: done_tx,
            }))
            .map_err(|_| AppError::AudioDevice("Thread audio indisponible".into()))?;

        done_rx
            .await
            .map_err(|_| AppError::AudioDevice("Le thread audio s'est arrêté de manière inattendue".into()))?
    }

    /// Demande au thread audio de rouvrir le flux sur un nouveau périphérique.
    pub async fn switch_device(&self, device_name: Option<String>) -> AppResult<()> {
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(AudioCommand::SwitchDevice { device_name, ack: ack_tx })
            .map_err(|_| AppError::AudioDevice("Thread audio indisponible".into()))?;

        ack_rx
            .await
            .map_err(|_| AppError::AudioDevice("Le thread audio s'est arrêté de manière inattendue".into()))?
    }
}

/// Boucle du thread audio dédié : consomme les commandes séquentiellement.
/// Tourne jusqu'à ce que tous les `Sender` (donc l'`AudioPlayer`) soient
/// abandonnés, ce qui se produit naturellement à la fermeture de l'application.
fn audio_thread_loop(mut state: AudioThreadState, command_rx: std_mpsc::Receiver<AudioCommand>) {
    while let Ok(command) = command_rx.recv() {
        match command {
            AudioCommand::Play(req) => {
                let result = play_blocking(&state.sink, req.samples, req.sample_rate, req.volume, req.pitch);
                req.done.send(result).ok();
            }
            AudioCommand::SwitchDevice { device_name, ack } => {
                let result = AudioThreadState::open(device_name.as_deref()).map(|new_state| {
                    state = new_state;
                });
                ack.send(result).ok();
            }
        }
    }
}

fn play_blocking(sink: &Sink, samples: Vec<i16>, sample_rate: u32, volume: f32, pitch: f32) -> AppResult<()> {
    // `pitch` in [-1.0, 1.0] -> facteur multiplicatif de fréquence déclarée.
    let adjusted_rate = ((sample_rate as f32) * (1.0 + pitch.clamp(-0.9, 2.0))).round() as u32;

    let source = RawPcmSource {
        samples: samples.into_iter(),
        sample_rate: adjusted_rate.max(4000),
    }
    .amplify(volume.clamp(0.0, 2.0));

    sink.append(source);
    sink.sleep_until_end(); // bloquant, mais on est déjà sur le thread audio dédié

    Ok(())
}
