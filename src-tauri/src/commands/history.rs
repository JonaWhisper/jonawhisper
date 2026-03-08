use crate::errors::AppError;
use crate::state::{AppState, HistoryEntry};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct HistoryPage {
    entries: Vec<HistoryEntry>,
    total: u32,
}

#[tauri::command]
pub fn get_history(query: String, limit: u32, cursor: Option<u64>, state: tauri::State<'_, Arc<AppState>>) -> Result<HistoryPage, AppError> {
    let entries = state.get_history(&query, limit, cursor)?;
    let total = state.history_count(&query)?;
    Ok(HistoryPage { entries, total })
}

#[tauri::command]
pub fn delete_history_entry(timestamp: u64, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.delete_history_entry(timestamp)?;
    Ok(())
}

#[tauri::command]
pub fn delete_history_day(day_timestamp: u64, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.delete_history_day(day_timestamp)?;
    Ok(())
}

#[tauri::command]
pub fn clear_history(state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.clear_history()?;
    Ok(())
}
