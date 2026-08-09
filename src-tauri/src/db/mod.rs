//! Persistance locale (historique des messages, statistiques agrégées).

mod repository;

pub use repository::{DailyStats, HistoryEntry, Repository, StatsSummary};
