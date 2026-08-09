//! Statistiques de la session courante, tenues en mémoire pour un accès
//! instantané par le tableau de bord (évite une requête SQLite à chaque
//! rafraîchissement de l'UI). Les statistiques long-terme/historiques
//! restent portées par [`crate::db::Repository::stats_summary`].

use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::Mutex;

#[derive(Debug, Clone, Serialize, Default)]
pub struct SessionStatsSnapshot {
    pub messages_read: u64,
    pub messages_ignored: u64,
    pub active_users_count: usize,
    pub total_reading_time_ms: u64,
}

pub struct SessionStats {
    messages_read: AtomicU64,
    messages_ignored: AtomicU64,
    total_reading_time_ms: AtomicU64,
    active_users: Mutex<HashSet<String>>,
}

impl SessionStats {
    pub fn new() -> Self {
        Self {
            messages_read: AtomicU64::new(0),
            messages_ignored: AtomicU64::new(0),
            total_reading_time_ms: AtomicU64::new(0),
            active_users: Mutex::new(HashSet::new()),
        }
    }

    pub fn record_read(&self, user_login: &str) {
        self.messages_read.fetch_add(1, Ordering::Relaxed);
        self.active_users.lock().insert(user_login.to_string());
    }

    pub fn record_ignored(&self, user_login: &str) {
        self.messages_ignored.fetch_add(1, Ordering::Relaxed);
        self.active_users.lock().insert(user_login.to_string());
    }

    pub fn record_reading_time(&self, duration_ms: u64) {
        self.total_reading_time_ms.fetch_add(duration_ms, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> SessionStatsSnapshot {
        SessionStatsSnapshot {
            messages_read: self.messages_read.load(Ordering::Relaxed),
            messages_ignored: self.messages_ignored.load(Ordering::Relaxed),
            active_users_count: self.active_users.lock().len(),
            total_reading_time_ms: self.total_reading_time_ms.load(Ordering::Relaxed),
        }
    }
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}
