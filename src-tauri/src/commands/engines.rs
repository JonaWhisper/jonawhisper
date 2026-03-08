use super::catalog;
use crate::errors::AppError;
use crate::events;
use crate::state::AppState;
use jona_engines::downloader;
use jona_engines::{EngineInfo, Language};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn get_engines() -> Vec<EngineInfo> {
    catalog().engine_infos()
}

#[tauri::command]
pub fn get_models(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, AppError> {
    let cat = catalog();
    let language = state.settings.lock().unwrap().selected_language.clone();
    let recommended_ids = cat.recommended_model_ids(&language);
    cat.all_models().into_iter().map(|m| {
        let downloaded = m.is_downloaded();
        let recommended = recommended_ids.contains(&m.id);
        let partial = if downloaded { None } else { downloader::partial_progress(&m) };
        let mut json = serde_json::to_value(&m)
            .map_err(|e| AppError::Other(e.to_string()))?;
        let obj = json.as_object_mut().unwrap();
        obj.insert("is_downloaded".into(), downloaded.into());
        obj.insert("recommended".into(), recommended.into());
        obj.insert("partial_progress".into(), serde_json::json!(partial));
        Ok(json)
    }).collect()
}

#[tauri::command]
pub async fn download_model_cmd(
    app: AppHandle,
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    let model = catalog()
        .model_by_id(&id)
        .ok_or_else(|| AppError::Other(format!("Model not found: {}", id)))?;

    let result = downloader::download_model(app.clone(), Arc::clone(&state.download), model).await;
    let _ = app.emit(events::MODELS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub async fn delete_model_cmd(app: AppHandle, id: String) -> Result<bool, AppError> {
    let result = tokio::task::spawn_blocking(move || {
        catalog()
            .model_by_id(&id)
            .is_some_and(|m| downloader::delete_model(&m))
    }).await.map_err(|e| AppError::Other(e.to_string()))?;
    let _ = app.emit(events::MODELS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub fn pause_download(id: String, state: tauri::State<'_, Arc<AppState>>) {
    let dl = state.download.lock().unwrap();
    if let Some(entry) = dl.active.get(&id) {
        entry.cancel_requested.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn cancel_download(app: AppHandle, id: String, state: tauri::State<'_, Arc<AppState>>) {
    let dl = state.download.lock().unwrap();
    let is_active = dl.active.contains_key(&id);
    if let Some(entry) = dl.active.get(&id) {
        entry.cancel_requested.store(true, Ordering::Relaxed);
        entry.delete_partial.store(true, Ordering::Relaxed);
    }
    drop(dl);

    // Also delete partial directly (handles paused/no-active-download case)
    if !is_active {
        if let Some(model) = catalog().model_by_id(&id) {
            downloader::delete_partial(&model);
        }
        let _ = app.emit(events::MODELS_CHANGED, ());
    }
}

#[tauri::command]
pub fn get_languages() -> Vec<Language> {
    catalog().supported_languages()
}
