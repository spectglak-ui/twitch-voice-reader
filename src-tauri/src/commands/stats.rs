//! Commandes Tauri pour les onglets "Tableau de bord" / "Historique" /
//! "Statistiques".

use crate::db::{HistoryEntry, StatsSummary};
use crate::stats::SessionStatsSnapshot;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_session_stats(state: State<'_, AppState>) -> SessionStatsSnapshot {
    state.session_stats.snapshot()
}

#[tauri::command]
pub fn get_stats_summary(
    days: u32,
    state: State<'_, AppState>,
) -> Result<StatsSummary, crate::error::AppError> {
    state.db.stats_summary(days)
}

#[tauri::command]
pub fn get_history(
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryEntry>, crate::error::AppError> {
    state.db.recent_history(limit)
}
