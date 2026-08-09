//! Serveur d'overlay pour OBS Studio / Streamlabs (Browser Source).
//!
//! ## Choix technique : serveur HTTP+WS local plutôt que fichier partagé
//!
//! Deux approches existent pour piloter un overlay depuis une application
//! desktop :
//! 1. Écrire un fichier JSON/HTML mis à jour à chaque message, lu par un
//!    Browser Source configuré en "rafraîchir toutes les X secondes".
//! 2. **Servir une page HTML statique + pousser les évènements en temps
//!    réel via WebSocket** (retenu ici).
//!
//! L'option 2 est celle utilisée par les overlays professionnels
//! (Streamlabs, StreamElements) : latence quasi nulle, pas de scintillement
//! au rafraîchissement, et une seule page à ajouter une fois dans OBS
//! (`http://127.0.0.1:<port>/overlay`) qui reste à jour automatiquement.

use crate::tts::TtsPlaybackEvent;
use axum::extract::ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
struct OverlayState {
    broadcaster: Arc<broadcast::Sender<TtsPlaybackEvent>>,
}

pub struct OverlayServer {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl OverlayServer {
    /// Démarre le serveur sur `port`, en s'abonnant directement au bus
    /// d'évènements TTS partagé de l'application (voir `AppState::tts_events`).
    /// Chaque client WebSocket (Browser Source OBS) obtient son propre
    /// abonnement via `broadcaster.subscribe()` dans `handle_socket`.
    pub async fn start(
        port: u16,
        broadcaster: Arc<broadcast::Sender<TtsPlaybackEvent>>,
    ) -> anyhow::Result<Self> {
        let state = OverlayState {
            broadcaster: broadcaster.clone(),
        };

        let app = Router::new()
            .route("/overlay", get(overlay_page))
            .route("/overlay/ws", get(ws_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tauri::async_runtime::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });
            if let Err(e) = server.await {
                tracing::error!("Erreur serveur overlay : {e}");
            }
        });

        Ok(Self {
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).ok();
        }
    }
}

impl Drop for OverlayServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<OverlayState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: OverlayState) {
    let mut rx = state.broadcaster.subscribe();
    while let Ok(evt) = rx.recv().await {
        let Ok(json) = serde_json::to_string(&evt) else {
            continue;
        };
        if socket.send(AxumWsMessage::Text(json)).await.is_err() {
            break; // client déconnecté (Browser Source rechargée/fermée)
        }
    }
}

/// Page HTML/CSS/JS minimaliste de l'overlay : fond transparent, affichage
/// du message en cours de lecture avec animation. Personnalisable par
/// l'utilisateur (le fichier peut être copié et édité librement, OBS ne
/// nécessite qu'une URL valide).
async fn overlay_page() -> impl IntoResponse {
    Html(include_str!("overlay_page.html"))
}
