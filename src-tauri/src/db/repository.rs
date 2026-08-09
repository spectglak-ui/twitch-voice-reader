//! Accès aux données locales (SQLite via `rusqlite` + pool `r2d2`).
//!
//! ## Pourquoi `rusqlite` + `r2d2` plutôt que `sqlx` ?
//!
//! | Critère | rusqlite (sync) + r2d2 | sqlx (async) |
//! |---|---|---|
//! | Complexité d'intégration Tauri | Faible (pool bloquant dans `spawn_blocking`) | Moyenne (runtime partagé, macros compile-time nécessitant une DB au build) |
//! | Pertinence pour une appli desktop mono-utilisateur | Élevée (pas de forte concurrence à gérer) | Overkill |
//! | Taille binaire / temps de compilation | Plus faible | Plus élevé (macros + dérivation) |
//!
//! Pour une base locale à faible concurrence (un seul processus, quelques
//! écritures par seconde au pic), un pool `rusqlite` synchrone exécuté via
//! `tokio::task::spawn_blocking` est plus simple à maintenir et suffisant en
//! performance. `sqlx` deviendrait pertinent si une synchronisation cloud
//! multi-appareils était ajoutée plus tard (roadmap Phase 4+).

use crate::error::AppResult;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

pub type DbPool = r2d2::Pool<SqliteConnectionManager>;

const SCHEMA_SQL: &str = include_str!("schema.sql");

#[derive(Clone)]
pub struct Repository {
    pool: DbPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub id: String,
    pub channel: String,
    pub username_login: String,
    pub display_name: String,
    pub role: String,
    pub text: String,
    pub was_read_aloud: bool,
    pub rejection_reason: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DailyStats {
    pub day: String,
    pub messages_read: i64,
    pub messages_ignored: i64,
    pub reading_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StatsSummary {
    pub total_messages_read: i64,
    pub total_messages_ignored: i64,
    pub total_reading_time_ms: i64,
    pub active_users_last_30_days: i64,
    pub daily_breakdown: Vec<DailyStats>,
}

impl Repository {
    pub fn open(app_data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(app_data_dir)?;
        let db_path = app_data_dir.join("history.sqlite3");
        let manager = SqliteConnectionManager::file(db_path).with_init(|c| {
            // WAL améliore fortement la concurrence lecture/écriture pour un
            // usage desktop (l'UI peut lire l'historique pendant qu'on insère).
            c.pragma_update(None, "journal_mode", "WAL")?;
            c.pragma_update(None, "foreign_keys", "ON")?;
            Ok(())
        });
        let pool = r2d2::Pool::builder().max_size(4).build(manager)?;

        {
            let conn = pool.get()?;
            conn.execute_batch(SCHEMA_SQL)?;
        }

        Ok(Self { pool })
    }

    pub fn insert_message(&self, entry: &HistoryEntry) -> AppResult<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO message_history
                (id, channel, username_login, display_name, role, text, was_read_aloud, rejection_reason, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id,
                entry.channel,
                entry.username_login,
                entry.display_name,
                entry.role,
                entry.text,
                entry.was_read_aloud as i64,
                entry.rejection_reason,
                entry.created_at_ms,
            ],
        )?;

        conn.execute(
            "INSERT INTO known_users (login, display_name, last_seen_ms, message_count)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(login) DO UPDATE SET
                display_name = excluded.display_name,
                last_seen_ms = excluded.last_seen_ms,
                message_count = message_count + 1",
            params![entry.username_login, entry.display_name, entry.created_at_ms],
        )?;

        let day = chrono::DateTime::from_timestamp_millis(entry.created_at_ms)
            .unwrap_or_else(chrono::Utc::now)
            .format("%Y-%m-%d")
            .to_string();

        let (read_delta, ignored_delta) = if entry.was_read_aloud { (1, 0) } else { (0, 1) };
        conn.execute(
            "INSERT INTO stats_daily (day, messages_read, messages_ignored, reading_time_ms)
             VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(day) DO UPDATE SET
                messages_read = messages_read + ?2,
                messages_ignored = messages_ignored + ?3",
            params![day, read_delta, ignored_delta],
        )?;

        Ok(())
    }

    pub fn add_reading_time(&self, duration_ms: i64) -> AppResult<()> {
        let conn = self.pool.get()?;
        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        conn.execute(
            "INSERT INTO stats_daily (day, messages_read, messages_ignored, reading_time_ms)
             VALUES (?1, 0, 0, ?2)
             ON CONFLICT(day) DO UPDATE SET reading_time_ms = reading_time_ms + ?2",
            params![day, duration_ms],
        )?;
        Ok(())
    }

    pub fn recent_history(&self, limit: u32) -> AppResult<Vec<HistoryEntry>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, channel, username_login, display_name, role, text, was_read_aloud, rejection_reason, created_at_ms
             FROM message_history ORDER BY created_at_ms DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                channel: row.get(1)?,
                username_login: row.get(2)?,
                display_name: row.get(3)?,
                role: row.get(4)?,
                text: row.get(5)?,
                was_read_aloud: row.get::<_, i64>(6)? != 0,
                rejection_reason: row.get(7)?,
                created_at_ms: row.get(8)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn stats_summary(&self, days: u32) -> AppResult<StatsSummary> {
        let conn = self.pool.get()?;
        let since_day = (chrono::Utc::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();

        let mut stmt = conn.prepare(
            "SELECT day, messages_read, messages_ignored, reading_time_ms
             FROM stats_daily WHERE day >= ?1 ORDER BY day ASC",
        )?;
        let daily_breakdown: Vec<DailyStats> = stmt
            .query_map(params![since_day], |row| {
                Ok(DailyStats {
                    day: row.get(0)?,
                    messages_read: row.get(1)?,
                    messages_ignored: row.get(2)?,
                    reading_time_ms: row.get(3)?,
                })
            })?
            .filter_map(Result::ok)
            .collect();

        let totals = conn.query_row(
            "SELECT COALESCE(SUM(messages_read),0), COALESCE(SUM(messages_ignored),0), COALESCE(SUM(reading_time_ms),0)
             FROM stats_daily",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        )?;

        let active_users: i64 = conn.query_row(
            "SELECT COUNT(*) FROM known_users WHERE last_seen_ms >= ?1",
            params![(chrono::Utc::now() - chrono::Duration::days(30)).timestamp_millis()],
            |row| row.get(0),
        )?;

        Ok(StatsSummary {
            total_messages_read: totals.0,
            total_messages_ignored: totals.1,
            total_reading_time_ms: totals.2,
            active_users_last_30_days: active_users,
            daily_breakdown,
        })
    }

    /// Purge l'historique plus ancien que `retention_days`, appelée
    /// périodiquement (tâche de fond quotidienne) selon `general.history_retention_days`.
    pub fn purge_older_than(&self, retention_days: u32) -> AppResult<usize> {
        let conn = self.pool.get()?;
        let cutoff_ms = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).timestamp_millis();
        let affected = conn.execute(
            "DELETE FROM message_history WHERE created_at_ms < ?1",
            params![cutoff_ms],
        )?;
        Ok(affected)
    }
}
