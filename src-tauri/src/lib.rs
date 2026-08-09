//! Assemblage de l'application.
//!
//! `run()` est appelé depuis `main.rs` (et depuis le point d'entrée mobile
//! si une cible iOS/Android est ajoutée plus tard, conformément au template
//! standard Tauri 2). Ce fichier :
//! 1. Initialise le logging structuré (`tracing`).
//! 2. Charge la configuration et ouvre la base de données locale.
//! 3. Construit tous les sous-systèmes (Twitch, TTS, audio, overlay).
//! 4. Démarre le **pipeline central** : chaîne Twitch -> filtres -> anti-spam
//!    -> historique DB -> file TTS -> évènements frontend/overlay.
//! 5. Enregistre les commandes Tauri et lance la boucle d'évènements.

mod audio;
mod commands;
mod config;
mod db;
mod error;
mod filters;
mod overlay;
mod state;
mod stats;
mod tts;
mod twitch;

use config::ConfigStore;
use filters::{AntiSpamEngine, AntiSpamVerdict, FilterEngine, FilterVerdict};
use state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex as AsyncMutex;
use tts::{PiperEngine, TtsQueue};
use twitch::{ConnectionManager, ManagerEvent, TokenStore};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logger unique : `tracing_subscriber` s'installe comme implémentation
    // globale du crate `log` (via la passerelle `tracing-log`, activée par
    // défaut). Enregistrer *aussi* `tauri-plugin-log` provoquait une
    // double initialisation du logger global du crate `log` — la seconde
    // tentative échouait avec `attempted to set a logger after the
    // logging system was already initialized`. Le crate `log` n'autorise
    // qu'un seul logger global pour toute la durée de vie du process ;
    // on ne garde donc que `tracing_subscriber`, déjà utilisé partout
    // dans le code via les macros `tracing::info!`/`warn!`/`error!`, et on
    // ne dépend plus de `tauri-plugin-log` (retiré aussi de `Cargo.toml`).
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        // Doit être enregistré en tout premier : empêche le lancement
        // d'une seconde instance (l'overlay HTTP écoute sur un port fixe
        // et deux instances tenteraient toutes deux de s'y lier, en plus
        // de se connecter en doublon au chat Twitch). Le callback
        // réactive et met au premier plan la fenêtre déjà ouverte plutôt
        // que d'ouvrir une seconde fenêtre.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                window.show().ok();
                window.set_focus().ok();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_handle = app.handle().clone();
            // `?` plutôt que `.expect(...)` : un échec ici (permissions
            // système, profil utilisateur corrompu, environnement exotique)
            // ne doit pas paniquer brutalement mais remonter comme une
            // erreur de démarrage normale, gérée par `.run().expect(...)`
            // au niveau de `main()` avec un message clair.
            let app_data_dir = app_handle.path().app_data_dir().map_err(|e| {
                crate::error::AppError::Internal(format!(
                    "Impossible de résoudre le répertoire de données applicatif : {e}"
                ))
            })?;

            let config_store = ConfigStore::load(&app_data_dir)?;
            let repository = Arc::new(db::Repository::open(&app_data_dir)?);

            // Synchronise l'état d'autodémarrage système avec la préférence
            // persistée (au cas où l'utilisateur l'aurait modifiée hors de
            // l'application, ou après une réinstallation).
            {
                use tauri_plugin_autostart::ManagerExt;
                let autolaunch = app_handle.autolaunch();
                let should_autostart = config_store.get().general.launch_on_system_startup;
                let is_enabled = autolaunch.is_enabled().unwrap_or(false);
                if should_autostart && !is_enabled {
                    autolaunch.enable().ok();
                } else if !should_autostart && is_enabled {
                    autolaunch.disable().ok();
                }
            }

            let piper_binary = resolve_piper_binary(&app_handle, &config_store);
            let piper_auto_install_dir = app_data_dir.join("piper");
            let voices_dir = app_data_dir.join("voices");
            std::fs::create_dir_all(&voices_dir).ok();
            seed_bundled_voices(&app_handle, &voices_dir);
            let piper = Arc::new(PiperEngine::new(piper_binary, piper_auto_install_dir, voices_dir));

            // Installation automatique de Piper en arrière-plan si aucun
            // binaire n'a pu être résolu au démarrage (ni chemin explicite,
            // ni ressources bundlées) — voir `tts::installer`. Ne bloque
            // jamais le lancement de l'application : lancée en tâche de
            // fond, avec repli explicite si l'utilisateur clique "Tester"
            // avant qu'elle ne se termine (voir `PiperEngine::synthesize`).
            let piper_for_autoinstall = piper.clone();
            let autoinstall_app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let progress_app = autoinstall_app.clone();
                let result = piper_for_autoinstall
                    .ensure_ready(move |progress| {
                        progress_app.emit("piper://install-progress", &progress).ok();
                    })
                    .await;
                if let Err(e) = result {
                    tracing::warn!("Installation automatique de Piper différée : {e}");
                    autoinstall_app
                        .emit(
                            "piper://install-progress",
                            &tts::InstallProgress::Error { message: e.to_string() },
                        )
                        .ok();
                }
            });

            let audio_device = config_store.get().audio.output_device_name.clone();
            let audio_player = Arc::new(audio::AudioPlayer::new(audio_device.as_deref())?);

            // Le Client ID n'est plus figé ici : il est résolu à la volée
            // (config utilisateur -> variable d'env -> placeholder) à
            // chaque tentative de connexion, voir
            // `twitch::auth::resolve_client_id` et `commands::auth::twitch_start_login`.
            // On se contente ici de journaliser un avertissement de
            // diagnostic si rien n'est configuré au démarrage.
            let initial_client_id = twitch::auth::resolve_client_id(&config_store.get().twitch);
            if !twitch::auth::is_client_id_configured(&initial_client_id) {
                tracing::warn!(
                    "Aucun Client ID Twitch configuré (ni dans les paramètres, ni via \
                     TWITCH_CLIENT_ID) : la connexion Twitch échouera tant qu'il ne sera pas \
                     renseigné dans l'onglet Connexions Twitch."
                );
            }

            // Le provider de token est interrogé à chaque tentative de
            // (re)connexion par `ConnectionManager` : il encapsule toute la
            // logique "où trouver le jeton actuel" sans coupler le module
            // Twitch au trousseau système.
            let token_provider = Arc::new(|| TokenStore::load_last().map(|t| t.access_token));
            // Le pseudo utilisé pour la connexion IRC est celui du compte
            // Twitch authentifié (le cahier des charges impose la connexion
            // OAuth comme prérequis MVP — pas de mode lecture anonyme).
            // Tant qu'aucun compte n'est authentifié, `token_provider`
            // retourne `None` et `ConnectionManager::connect` restera en
            // attente sans tenter de se connecter (voir connection_manager.rs).
            let bot_login = TokenStore::load_last()
                .map(|t| t.login)
                .unwrap_or_default();

            let (connection_manager, mut manager_events_rx) =
                ConnectionManager::new(bot_login, token_provider);
            let connection_manager = Arc::new(connection_manager);

            let (tts_events_tx, _) = tokio::sync::broadcast::channel(128);
            let tts_events_tx = Arc::new(tts_events_tx);
            let session_stats = Arc::new(stats::SessionStats::new());

            let tts_queue = TtsQueue::spawn(
                piper.clone(),
                audio_player.clone(),
                config_store.clone(),
                repository.clone(),
                session_stats.clone(),
                (*tts_events_tx).clone(),
            );

            let app_state = AppState {
                config: config_store.clone(),
                db: repository.clone(),
                connection_manager: connection_manager.clone(),
                piper,
                audio_player,
                tts_queue: Arc::new(AsyncMutex::new(Some(tts_queue))),
                anti_spam: Arc::new(parking_lot::Mutex::new(AntiSpamEngine::new())),
                session_stats,
                overlay_server: Arc::new(AsyncMutex::new(None)),
                tts_events: tts_events_tx.clone(),
            };
            app.manage(app_state);

            // --- Pipeline central : Twitch -> filtres -> anti-spam -> TTS ---
            let pipeline_app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = manager_events_rx.recv().await {
                    match event {
                        ManagerEvent::StatusChanged { channel, status } => {
                            pipeline_app
                                .emit("twitch://status", serde_json::json!({ "channel": channel, "status": status }))
                                .ok();
                        }
                        ManagerEvent::Notice { channel, text } => {
                            pipeline_app
                                .emit("twitch://notice", serde_json::json!({ "channel": channel, "text": text }))
                                .ok();
                        }
                        ManagerEvent::ChatMessage(message) => {
                            process_incoming_message(&pipeline_app, message).await;
                        }
                    }
                }
            });

            // --- Rediffusion des évènements TTS vers la fenêtre principale ---
            let ui_app = app_handle.clone();
            let mut ui_tts_rx = tts_events_tx.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(evt) = ui_tts_rx.recv().await {
                    ui_app.emit("tts://event", &evt).ok();
                }
            });

            // --- Reconnexion automatique des chaînes marquées `enabled` ---
            let startup_manager = connection_manager.clone();
            let startup_channels = config_store.get().channels;
            tauri::async_runtime::spawn(async move {
                for channel in startup_channels.into_iter().filter(|c| c.enabled) {
                    startup_manager.connect(&channel.login).await.ok();
                }
            });

            // --- Purge périodique de l'historique selon la rétention configurée ---
            let purge_repository = repository.clone();
            let purge_config = config_store.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
                loop {
                    interval.tick().await;
                    let retention_days = purge_config.get().general.history_retention_days;
                    let repo_for_purge = purge_repository.clone();
                    let purge_result = tauri::async_runtime::spawn_blocking(move || {
                        repo_for_purge.purge_older_than(retention_days)
                    })
                    .await;
                    if let Ok(Ok(deleted)) = purge_result {
                        if deleted > 0 {
                            tracing::info!("Purge historique : {deleted} message(s) supprimé(s)");
                        }
                    }
                }
            });

            // --- Démarrage minimisé dans la zone de notification -----------
            // La fenêtre est déclarée visible par défaut (`tauri.conf.json`) ;
            // on la masque explicitement ici si l'utilisateur l'a demandé,
            // et on relie un clic sur l'icône de la zone de notification à
            // sa réaffichage (comportement standard des apps de streaming).
            if let Some(main_window) = app_handle.get_webview_window("main") {
                if config_store.get().general.start_minimized_to_tray {
                    main_window.hide().ok();
                }
            }
            if let Some(tray) = app_handle.tray_by_id("main") {
                let tray_window_handle = app_handle.clone();
                tray.on_tray_icon_event(move |_tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        if let Some(window) = tray_window_handle.get_webview_window("main") {
                            window.show().ok();
                            window.set_focus().ok();
                        }
                    }
                });
            }

            // --- Nettoyage périodique de l'état anti-spam (mémoire bornée) ---
            let prune_anti_spam = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
                loop {
                    interval.tick().await;
                    let state = prune_anti_spam.state::<AppState>();
                    state
                        .anti_spam
                        .lock()
                        .prune_stale_entries(std::time::Duration::from_secs(3600));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::twitch_start_login,
            commands::auth::twitch_logout,
            commands::auth::twitch_current_account,
            commands::twitch::twitch_connect_channel,
            commands::twitch::twitch_disconnect_channel,
            commands::twitch::twitch_list_connections,
            commands::config::get_config,
            commands::config::update_twitch_config,
            commands::config::update_tts_config,
            commands::config::update_audio_config,
            commands::config::update_filters_config,
            commands::config::update_anti_spam_config,
            commands::config::update_overlay_config,
            commands::config::update_general_config,
            commands::config::set_user_voice_assignment,
            commands::config::set_role_voice_assignment,
            commands::config::export_config,
            commands::config::import_config,
            commands::config::reset_config,
            commands::tts::tts_list_installed_voices,
            commands::tts::tts_check_installation,
            commands::tts::tts_ensure_installed,
            commands::tts::tts_test_voice,
            commands::tts::audio_list_output_devices,
            commands::tts::audio_switch_output_device,
            commands::stats::get_session_stats,
            commands::stats::get_stats_summary,
            commands::stats::get_history,
            commands::overlay::overlay_start,
            commands::overlay::overlay_stop,
            commands::overlay::overlay_is_running,
        ])
        .run(tauri::generate_context!())
        .expect("erreur lors du démarrage de Twitch Voice Reader");
}

/// Applique filtres + anti-spam à un message entrant, journalise le
/// résultat en base, met à jour les statistiques de session, et pousse le
/// message vers la file TTS si toutes les conditions sont réunies. Émet
/// systématiquement le message vers l'UI (lu ou non) pour que l'onglet
/// "Connexions"/tableau de bord affiche le flux complet du chat.
async fn process_incoming_message(app: &tauri::AppHandle, message: twitch::ChatMessage) {
    let state = app.state::<AppState>();
    let config = state.config.get();

    let filter_verdict = FilterEngine::evaluate(&message, &config.filters);

    let (should_read, rejection_reason, occurrence_count) = match filter_verdict {
        FilterVerdict::Rejected(reason) => (false, Some(reason), 1),
        FilterVerdict::Accepted => {
            let verdict = {
                let mut engine = state.anti_spam.lock();
                engine.evaluate(&message.username_login, &message.text_for_tts, &config.anti_spam)
            };
            match verdict {
                AntiSpamVerdict::Allow => (true, None, 1),
                AntiSpamVerdict::GroupedDuplicate { occurrence_count } => (true, None, occurrence_count),
                AntiSpamVerdict::RepetitionThresholdExceeded | AntiSpamVerdict::RateLimited => {
                    (false, None, 1)
                }
            }
        }
    };

    if should_read {
        state.session_stats.record_read(&message.username_login);
        if let Some(queue) = state.tts_queue.lock().await.as_ref() {
            queue.try_enqueue(tts::QueuedMessage {
                message: message.clone(),
                occurrence_count,
            });
        }
    } else {
        state.session_stats.record_ignored(&message.username_login);
    }

    let history_entry = db::HistoryEntry {
        id: message.id.clone(),
        channel: message.channel.clone(),
        username_login: message.username_login.clone(),
        display_name: message.display_name.clone(),
        role: message.role.as_str().to_string(),
        text: message.text.clone(),
        was_read_aloud: should_read,
        rejection_reason: rejection_reason.map(|r| format!("{r:?}")),
        created_at_ms: message.timestamp_ms,
    };
    // `rusqlite` est une API bloquante. `process_incoming_message` s'exécute
    // sur le pool de threads du runtime async partagé par TOUTES les
    // connexions Twitch actives et par la lecture audio ; y exécuter une
    // écriture disque synchrone directement retarderait le traitement des
    // messages des autres chaînes le temps de l'I/O (peu sensible avec
    // SQLite/WAL en usage normal, mais mesurable sur un chat à fort débit
    // ou un disque lent/réseau). `spawn_blocking` délègue l'appel à un
    // thread dédié aux tâches bloquantes, sans jamais geler le runtime async.
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db.insert_message(&history_entry).ok();
    });

    app.emit(
        "twitch://message",
        serde_json::json!({ "message": message, "wasReadAloud": should_read }),
    )
    .ok();
}

/// Résout le chemin du binaire Piper *déjà connu* au démarrage, sans
/// déclencher de téléchargement (ça, c'est le rôle de
/// `PiperEngine::ensure_ready`, appelé séparément). Priorité :
/// 1. Chemin explicite en configuration — seulement s'il existe réellement
///    sur disque (un chemin configuré puis effacé/déplacé ne doit pas être
///    traité comme valide silencieusement) ;
/// 2. Ressources bundlées de l'application (`resources/piper/`), pour les
///    builds officiels où Piper est packagé dans l'installeur ;
/// 3. `None` — dans ce cas, `PiperEngine::ensure_ready` télécharge et
///    installe automatiquement Piper au premier besoin (voir `lib.rs::run`,
///    tâche de fond lancée juste après cet appel, et `tts/installer.rs`).
///
/// Volontairement **pas** de repli sur le simple nom `piper`/`piper.exe`
/// résolu via le `PATH` système : un tel repli ne peut pas être vérifié à
/// bas coût (il faudrait tenter de l'exécuter), et le traiter comme
/// "trouvé" sans vérification désactiverait l'installation automatique
/// pour quiconque n'a pas Piper sur son `PATH` — exactement le problème
/// initial rencontré (aucune erreur claire, aucune action corrective).
fn resolve_piper_binary(app: &tauri::AppHandle, config: &ConfigStore) -> Option<std::path::PathBuf> {
    if let Some(explicit) = config.get().tts.piper_binary_path {
        let path = std::path::PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
        tracing::warn!(
            "Chemin Piper configuré mais introuvable sur disque ({}), ignoré.",
            path.display()
        );
    }

    let binary_name = if cfg!(target_os = "windows") {
        "piper.exe"
    } else {
        "piper"
    };

    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("piper").join(binary_name);
        if bundled.is_file() {
            return Some(bundled);
        }
    }

    None
}

/// Copie les voix pré-embarquées (`resources/piper/voices/`, packagées
/// avec l'installeur) vers le dossier utilisateur inscriptible
/// (`app_data_dir/voices`) lors du tout premier lancement. Les téléchargements
/// ultérieurs de voix additionnelles depuis l'onglet "Voix et TTS" écrivent
/// directement dans ce même dossier utilisateur, jamais dans les ressources
/// bundlées (souvent en lecture seule selon la plateforme).
fn seed_bundled_voices(app: &tauri::AppHandle, voices_dir: &std::path::Path) {
    let Ok(resource_dir) = app.path().resource_dir() else {
        return;
    };
    let bundled_voices_dir = resource_dir.join("piper").join("voices");
    let Ok(entries) = std::fs::read_dir(&bundled_voices_dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let src = entry.path();
        let Some(file_name) = src.file_name() else { continue };
        let dest = voices_dir.join(file_name);
        if !dest.exists() {
            std::fs::copy(&src, &dest).ok();
        }
    }
}
