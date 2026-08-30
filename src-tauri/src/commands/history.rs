use crate::errors::AppError;
use crate::state::AppState;
use jona_types::HistoryEntry;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct HistoryPage {
    entries: Vec<HistoryEntry>,
    total: u32,
}

#[tauri::command]
pub fn get_history(query: String, limit: u32, cursor: Option<u64>, state: tauri::State<'_, Arc<AppState>>) -> Result<HistoryPage, AppError> {
    let entries = state.history.get(&query, limit, cursor)?;
    let total = state.history.count(&query)?;
    Ok(HistoryPage { entries, total })
}

#[tauri::command]
pub fn set_history_correction(timestamp: u64, corrected: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.history.set_correction(timestamp, &corrected)?;
    Ok(())
}

#[tauri::command]
pub fn set_history_rating(timestamp: u64, rating: i8, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.history.set_rating(timestamp, rating)?;
    Ok(())
}

#[tauri::command]
pub fn delete_history_entry(timestamp: u64, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.history.delete_entry(timestamp)?;
    Ok(())
}

#[tauri::command]
pub fn delete_history_day(day_timestamp: u64, state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.history.delete_day(day_timestamp)?;
    Ok(())
}

#[tauri::command]
pub fn clear_history(state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
    state.history.clear()?;
    Ok(())
}
