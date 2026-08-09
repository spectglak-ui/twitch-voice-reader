//! Intégration Twitch : authentification, connexion IRC, gestion multi-chaînes.

pub mod auth;
pub mod connection_manager;
pub mod irc_client;
pub mod message;
pub mod token_store;

pub use connection_manager::{ConnectionManager, ConnectionStatus, ManagerEvent};
pub use message::{ChatMessage, TwitchRole};
pub use token_store::TokenStore;
