use crate::audio;
use crate::events;
use crate::platform::hotkey;
use crate::platform::paste;
use crate::platform;
use crate::post_processor;
use crate::state::AppState;
use crate::transcriber;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Generation counter — incremented on each recording start, used to prevent
/// stale delayed pill closes (from error display) from killing a freshly opened pill.
static PILL_CLOSE_GENERATION: AtomicU64 = AtomicU64::new(0);

// -- Timing constants --

const SHORT_TAP_MS: u64 = 300;
const DOUBLE_TAP_MS: u64 = 500;
const ERROR_DISPLAY_MS: u64 = 800;
const SPECTRUM_INTERVAL_MS: u64 = 33;
const ORPHAN_CLEANUP_SECS: u64 = 300;

// -- Audio commands for the dedicated audio thread --

pub enum AudioCmd {
    StartRecording { device_uid: Option<String> },
    StopRecording,
    GetSpectrum,
    StartMicTest { device_uid: Option<String> },
    StopMicTest,
}

pub enum AudioReply {
    Started,
    Stopped { path: Option<std::path::PathBuf> },
}

// -- Recording state (Send-safe, does not hold AudioRecorder) --

pub struct RecordingState {
    key_down_time: Option<Instant>,
    last_short_tap_time: Option<Instant>,
    audio_tx: crossbeam_channel::Sender<AudioCmd>,
    audio_rx: crossbeam_channel::Receiver<AudioReply>,
}

// -- Recording lifecycle --

pub fn start_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if rt.is_recording {
            return;
        }
        // Cancel mic test if running
        if rt.mic_testing {
            rt.mic_testing = false;
            let _ = rec.audio_tx.send(AudioCmd::StopMicTest);
            let _ = app.emit(crate::events::MIC_TEST_STOPPED, ());
        }
        rt.is_recording = true;
        rt.transcription_cancelled = false;
    }
    rec.key_down_time = Some(Instant::now());
    PILL_CLOSE_GENERATION.fetch_add(1, Ordering::SeqCst);

    let (device_uid, ducking_enabled, ducking_level) = {
        let s = state.settings.lock().unwrap();
        (s.selected_input_device_uid.clone(), s.audio_ducking_enabled, s.audio_ducking_level)
    };
    if ducking_enabled {
        platform::audio_ducking::duck_volume(ducking_level);
    }
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    let _ = rec.audio_rx.recv();

    platform::play_sound("Tink");
    crate::tray::open_pill_window(app);
    crate::tray::set_tray_state(app, "recording");
    let _ = app.emit(crate::events::PILL_MODE, "recording");
    let _ = app.emit(crate::events::RECORDING_STARTED, ());
}

pub fn stop_recording_and_enqueue(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &mut RecordingState,
) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if !rt.is_recording {
            return;
        }
        rt.is_recording = false;
    }

    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
    platform::audio_ducking::restore_volume();
    let audio_path = match rec.audio_rx.recv() {
        Ok(AudioReply::Stopped { path }) => path,
        _ => None,
    };

    let held_duration = rec.key_down_time.map(|t| t.elapsed());
    rec.key_down_time = None;
    let is_short_tap = held_duration
        .map(|d| d < Duration::from_millis(SHORT_TAP_MS))
        .unwrap_or(false);

    if is_short_tap {
        handle_short_tap(app, state, rec, audio_path);
        return;
    }

    rec.last_short_tap_time = None;

    let audio_path = match audio_path {
        Some(p) => p,
        None => {
            log::warn!("No audio file produced, closing pill");
            crate::tray::close_pill_window(app);
            let _ = app.emit(crate::events::RECORDING_STOPPED, ());
            return;
        }
    };

    platform::play_sound("Pop");

    let count = state.enqueue(audio_path);
    let _ = app.emit(
        crate::events::RECORDING_STOPPED,
        serde_json::json!({ "queue_count": count }),
    );
    let _ = app.emit(crate::events::PILL_MODE, "transcribing");
    crate::tray::set_tray_state(app, "transcribing");

    let app_clone = app.clone();
    let state_clone = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        process_next_in_queue(&app_clone, &state_clone).await;
    });
}

fn handle_short_tap(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &mut RecordingState,
    audio_path: Option<std::path::PathBuf>,
) {
    if let Some(ref path) = audio_path {
        let _ = std::fs::remove_file(path);
    }

    // Double-tap: cancel transcription
    if let Some(last) = rec.last_short_tap_time {
        if last.elapsed() < Duration::from_millis(DOUBLE_TAP_MS) {
            rec.last_short_tap_time = None;
            cancel_transcription(app, state);
            return;
        }
    }
    rec.last_short_tap_time = Some(Instant::now());

    let rt = state.runtime.lock().unwrap();
    let is_transcribing = rt.is_transcribing;
    let queue_empty = rt.queue.is_empty();
    drop(rt);

    if !is_transcribing && queue_empty {
        crate::tray::close_pill_window(app);
    } else {
        let _ = app.emit(crate::events::PILL_MODE, "transcribing");
        crate::tray::set_tray_state(app, "transcribing");
    }
    let _ = app.emit(crate::events::RECORDING_STOPPED, ());
}

// -- Transcription queue processing --

pub async fn process_next_in_queue(app: &AppHandle, state: &Arc<AppState>) {
    loop {
        {
            let mut rt = state.runtime.lock().unwrap();
            if rt.is_transcribing {
                return;
            }
            if rt.queue.is_empty() {
                return;
            }
            rt.is_transcribing = true;
        }
        let audio_path = match state.dequeue() {
            Some(p) => p,
            None => {
                state.runtime.lock().unwrap().is_transcribing = false;
                return;
            }
        };

        let _ = app.emit(
            crate::events::TRANSCRIPTION_STARTED,
            serde_json::json!({ "queue_count": state.queue_count() }),
        );

        let had_error = run_transcription(app, state, &audio_path).await;
        let _ = std::fs::remove_file(&audio_path);
        state.runtime.lock().unwrap().is_transcribing = false;

        if had_error {
            show_error_then_close(app);
            return;
        }

        // Stop if cancelled or queue is empty
        let rt = state.runtime.lock().unwrap();
        if rt.transcription_cancelled || rt.queue.is_empty() {
            break;
        }
    }

    let mut rt = state.runtime.lock().unwrap();
    if !rt.is_recording {
        rt.last_paste_had_content = false;
        drop(rt);
        crate::tray::close_pill_window(app);
    }
}

async fn run_transcription(
    app: &AppHandle,
    state: &Arc<AppState>,
    audio_path: &std::path::Path,
) -> bool {
    let state_clone = Arc::clone(state);
    let path = audio_path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        transcriber::transcribe(&state_clone, &path)
    })
    .await;

    match result {
        Ok(Ok(text)) => {
            if state.runtime.lock().unwrap().transcription_cancelled {
                log::info!("Transcription result discarded (cancelled)");
                return false;
            }
            handle_transcription_result(app, state, &text).await;
            false
        }
        Ok(Err(e)) => {
            log::error!("Transcription error: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                events::TRANSCRIPTION_ERROR,
                serde_json::json!({ "error": e.to_string() }),
            );
            true
        }
        Err(e) => {
            log::error!("Transcription task panicked: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                events::TRANSCRIPTION_ERROR,
                serde_json::json!({ "error": "Internal error" }),
            );
            true
        }
    }
}

async fn handle_transcription_result(app: &AppHandle, state: &Arc<AppState>, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    // Read settings once
    let (lang, hall_filter, text_cleanup_enabled, cleanup_model_id,
         llm_model, llm_max_tokens, providers) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_language.clone(),
            s.hallucination_filter_enabled,
            s.text_cleanup_enabled,
            s.cleanup_model_id.clone(),
            s.llm_model.clone(),
            s.llm_max_tokens,
            s.providers.clone(),
        )
    };

    // Step 1: preprocess (hallucination filter + dictation commands)
    let mut processed = {
        let opts = post_processor::PostProcessOptions {
            hallucination_filter: hall_filter,
        };
        post_processor::preprocess(trimmed, &lang, &opts)
    };

    if processed.trim().is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    // Step 2: cleanup based on selected model
    let mut effective_cleanup_model_id = String::new();
    if text_cleanup_enabled && !cleanup_model_id.is_empty() {
        if cleanup_model_id.starts_with("bert-punctuation:") {
            // BERT punctuation restoration (finalize after)
            let state_clone = Arc::clone(state);
            let text_for_bert = processed.clone();
            let model_id = cleanup_model_id.clone();
            match tokio::task::spawn_blocking(move || {
                run_bert_punctuation(&state_clone, &text_for_bert, &model_id)
            }).await {
                Ok(Ok(punctuated)) => {
                    log::info!("BERT punctuation: {} → {}", processed.len(), punctuated.len());
                    processed = punctuated;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Ok(Err(e)) => {
                    log::warn!("BERT punctuation failed, using preprocessed result: {}", e);
                }
                Err(e) => {
                    log::warn!("BERT punctuation task panicked: {}", e);
                }
            }
            processed = post_processor::finalize(&processed);
        } else {
            // LLM cleanup (cloud or local) — finalize before
            processed = post_processor::finalize(&processed);

            // Auto-scale max_tokens based on input length (1 token ≈ 3 chars),
            // capped by user's hard cap setting (default 4096)
            let effective_max_tokens = std::cmp::min(
                llm_max_tokens,
                std::cmp::max((processed.len() as u32) / 3 + 64, 128),
            );

            let llm_result = if let Some(cloud_provider_id) = cleanup_model_id.strip_prefix("cloud:") {
                if let Some(provider) = providers.iter().find(|p| p.id == cloud_provider_id) {
                    crate::llm_cleanup::cleanup_text(&processed, &lang, provider, &llm_model, effective_max_tokens).await
                } else {
                    log::warn!("Cloud LLM provider '{}' not found", cloud_provider_id);
                    Err(crate::llm_prompt::LlmError::NotConfigured)
                }
            } else {
                // Local LLM (llama:*)
                let state_clone = Arc::clone(state);
                let text_for_llm = processed.clone();
                let lang_for_llm = lang.clone();
                let model_id = cleanup_model_id.clone();
                match tokio::task::spawn_blocking(move || {
                    run_local_llm_cleanup(&state_clone, &text_for_llm, &lang_for_llm, &model_id, effective_max_tokens)
                }).await {
                    Ok(result) => result,
                    Err(e) => Err(crate::llm_prompt::LlmError::Inference(format!("LLM task panicked: {}", e))),
                }
            };

            match llm_result {
                Ok(cleaned) => {
                    log::info!("LLM cleanup: {} → {}", processed.len(), cleaned.len());
                    processed = cleaned;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Err(e) => {
                    // Always fall back to finalized text — never lose the user's dictation
                    log::warn!("LLM cleanup failed (fallback to raw): {}", e);
                }
            }
        }
    } else {
        // No cleanup: just finalize
        processed = post_processor::finalize(&processed);
    }

    // Check cancel flag before pasting (cancel may arrive during LLM cleanup)
    if state.runtime.lock().unwrap().transcription_cancelled {
        log::info!("Transcription cancelled before paste, discarding");
        return;
    }

    // Add a leading space when pasting consecutive results (queued recordings)
    let needs_separator = state.runtime.lock().unwrap().last_paste_had_content;
    let paste_text = if needs_separator {
        format!(" {}", processed)
    } else {
        processed.clone()
    };
    // Run paste on a blocking thread to avoid stalling the async runtime (thread::sleep inside)
    let app_for_paste = app.clone();
    let _ = tokio::task::spawn_blocking(move || {
        paste::paste_text(&app_for_paste, &paste_text);
    })
    .await;
    state.runtime.lock().unwrap().last_paste_had_content = true;
    let (model_id, language) = {
        let s = state.settings.lock().unwrap();
        (s.selected_model_id.clone(), s.selected_language.clone())
    };
    state.add_history(processed.clone(), model_id, language, effective_cleanup_model_id.clone(), hall_filter);
    platform::play_sound("Glass");

    let _ = app.emit(
        crate::events::TRANSCRIPTION_COMPLETE,
        serde_json::json!({
            "text": processed,
            "cleanup_model_id": effective_cleanup_model_id,
            "hallucination_filter": hall_filter,
        }),
    );
}

// -- BERT punctuation --

fn run_bert_punctuation(
    state: &Arc<AppState>,
    text: &str,
    punctuation_model_id: &str,
) -> Result<String, String> {
    // Resolve model: use explicit ID or auto-select first downloaded punctuation model
    let catalog = crate::engines::EngineCatalog::new();
    let model = if punctuation_model_id.is_empty() {
        // Auto-select first downloaded punctuation model
        catalog.all_models().into_iter()
            .find(|m| m.engine_id == "bert-punctuation" && m.is_downloaded())
            .ok_or_else(|| "No punctuation model downloaded".to_string())?
    } else {
        catalog.model_by_id(punctuation_model_id)
            .ok_or_else(|| format!("Punctuation model not found: {}", punctuation_model_id))?
    };

    if !model.is_downloaded() {
        return Err(format!("Punctuation model not downloaded: {}", model.id));
    }

    let model_path = model.local_path();

    // Load or reuse cached BertContext
    let mut ctx_guard = state.bert_context.lock().unwrap();
    if ctx_guard.as_ref().map_or(true, |ctx| ctx.model_id() != model.id) {
        log::info!("Loading BERT punctuation model: {}", model.id);
        let ctx = crate::bert_punctuation::BertContext::load(&model_path, &model.id)?;
        *ctx_guard = Some(ctx);
    }

    let ctx = ctx_guard.as_mut().unwrap();
    crate::bert_punctuation::restore_punctuation(ctx, text)
}

// -- Local LLM cleanup --

fn run_local_llm_cleanup(
    state: &Arc<AppState>,
    text: &str,
    language: &str,
    llm_local_model_id: &str,
    max_tokens: u32,
) -> Result<String, crate::llm_prompt::LlmError> {
    use crate::llm_prompt::LlmError;

    if llm_local_model_id.is_empty() {
        return Err(LlmError::NotConfigured);
    }

    // Resolve model path from catalog
    let catalog = crate::engines::EngineCatalog::new();
    let model = catalog.model_by_id(llm_local_model_id)
        .ok_or_else(|| LlmError::Inference(format!("LLM model not found: {}", llm_local_model_id)))?;

    if !model.is_downloaded() {
        return Err(LlmError::Inference(format!("LLM model not downloaded: {}", llm_local_model_id)));
    }

    let model_path = model.local_path();

    // Load or reuse cached LlmContext
    let mut ctx_guard = state.llm_context.lock().unwrap();
    if ctx_guard.as_ref().map_or(true, |ctx| ctx.model_id() != llm_local_model_id) {
        log::info!("Loading local LLM model: {}", llm_local_model_id);
        let ctx = crate::llm_local::LlmContext::load(&model_path, llm_local_model_id)?;
        *ctx_guard = Some(ctx);
    }

    let ctx = ctx_guard.as_ref().unwrap();
    crate::llm_local::cleanup_text(ctx, text, language, max_tokens as usize)
}

// -- Cleanup helpers --

fn show_error_then_close(app: &AppHandle) {
    let _ = app.emit(crate::events::PILL_MODE, "error");
    let gen = PILL_CLOSE_GENERATION.load(Ordering::SeqCst);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ERROR_DISPLAY_MS)).await;
        // Only close if no new recording started since the error
        if PILL_CLOSE_GENERATION.load(Ordering::SeqCst) == gen {
            crate::tray::close_pill_window(&app_clone);
        }
    });
}

fn cancel_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if !rt.is_recording {
            return;
        }
        rt.is_recording = false;
    }

    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
    platform::audio_ducking::restore_volume();
    if let Ok(AudioReply::Stopped { path: Some(path) }) = rec.audio_rx.recv() {
        let _ = std::fs::remove_file(&path);
    }
    rec.key_down_time = None;
    rec.last_short_tap_time = None;

    platform::play_sound("Funk");
    let _ = app.emit(crate::events::RECORDING_STOPPED, ());
    show_error_then_close(app);
}

fn cancel_transcription(app: &AppHandle, state: &Arc<AppState>) {
    while let Some(path) = state.dequeue() {
        let _ = std::fs::remove_file(&path);
    }
    {
        let mut rt = state.runtime.lock().unwrap();
        rt.transcription_cancelled = true;
        rt.last_paste_had_content = false;
    }
    platform::play_sound("Funk");
    let _ = app.emit(crate::events::TRANSCRIPTION_CANCELLED, ());
    show_error_then_close(app);
}

pub fn cleanup_orphan_audio_files() {
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(ORPHAN_CLEANUP_SECS);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("whisper_dictate_") && name.ends_with(".wav") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(entry.path());
                            log::info!("Cleaned orphan audio: {}", name);
                        }
                    }
                }
            }
        }
    }
}

// -- Thread spawning --

/// Return type for spawn_audio_thread: (cmd_tx, spectrum_data, reply_rx, stream_error).
type AudioThreadHandles = (
    crossbeam_channel::Sender<AudioCmd>,
    Arc<std::sync::Mutex<Vec<f32>>>,
    crossbeam_channel::Receiver<AudioReply>,
    Arc<AtomicBool>,
);

/// Spawns the dedicated audio thread (cpal::Stream is not Send).
pub fn spawn_audio_thread() -> AudioThreadHandles {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let (reply_tx, reply_rx) = crossbeam_channel::unbounded::<AudioReply>();
    let spectrum_data = Arc::new(std::sync::Mutex::new(vec![0.0f32; 12]));
    let spectrum_clone = spectrum_data.clone();

    let stream_error = Arc::new(AtomicBool::new(false));
    let stream_error_clone = Arc::clone(&stream_error);

    std::thread::spawn(move || {
        let mut recorder = audio::AudioRecorder::new(stream_error_clone);
        loop {
            match cmd_rx.recv() {
                Ok(AudioCmd::StartRecording { device_uid }) => {
                    recorder.start_recording(device_uid.as_deref());
                    let _ = reply_tx.send(AudioReply::Started);
                }
                Ok(AudioCmd::StopRecording) => {
                    let path = recorder.stop_recording();
                    let _ = reply_tx.send(AudioReply::Stopped { path });
                }
                Ok(AudioCmd::GetSpectrum) => {
                    let s = recorder.get_spectrum();
                    *spectrum_clone.lock().unwrap() = s.clone();
                }
                Ok(AudioCmd::StartMicTest { device_uid }) => {
                    recorder.start_recording(device_uid.as_deref());
                    // No reply — fire-and-forget for mic test
                }
                Ok(AudioCmd::StopMicTest) => {
                    if let Some(path) = recorder.stop_recording() {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                Err(_) => break,
            }
        }
    });

    (cmd_tx, spectrum_data, reply_rx, stream_error)
}

pub fn spawn_hotkey_handler(
    hotkey_rx: crossbeam_channel::Receiver<hotkey::HotkeyEvent>,
    app: AppHandle,
    state: Arc<AppState>,
    rec_state: Arc<std::sync::Mutex<RecordingState>>,
) {
    std::thread::spawn(move || loop {
        match hotkey_rx.recv() {
            Ok(hotkey::HotkeyEvent::KeyDown) => {
                let mode = state.settings.lock().unwrap().recording_mode.clone();
                let is_recording = state.runtime.lock().unwrap().is_recording;
                let mut rec = rec_state.lock().unwrap();
                if mode == "toggle" && is_recording {
                    stop_recording_and_enqueue(&app, &state, &mut rec);
                } else {
                    start_recording(&app, &state, &mut rec);
                }
            }
            Ok(hotkey::HotkeyEvent::KeyUp) => {
                let mode = state.settings.lock().unwrap().recording_mode.clone();
                if mode != "toggle" {
                    let mut rec = rec_state.lock().unwrap();
                    stop_recording_and_enqueue(&app, &state, &mut rec);
                }
            }
            Ok(hotkey::HotkeyEvent::CancelPressed) => {
                let rt = state.runtime.lock().unwrap();
                let is_recording = rt.is_recording;
                let is_transcribing = rt.is_transcribing;
                let has_queue = !rt.queue.is_empty();
                drop(rt);

                if is_recording {
                    log::info!("Cancel shortcut pressed during recording, discarding");
                    let mut rec = rec_state.lock().unwrap();
                    cancel_recording(&app, &state, &mut rec);
                } else if is_transcribing || has_queue {
                    log::info!("Cancel shortcut pressed, cancelling transcription");
                    cancel_transcription(&app, &state);
                }
            }
            Ok(hotkey::HotkeyEvent::CaptureUpdate { modifiers, key_code }) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_UPDATE, serde_json::json!({
                    "modifiers": modifiers,
                    "key_code": key_code,
                }));
            }
            Ok(hotkey::HotkeyEvent::CaptureComplete(shortcut)) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_COMPLETE, serde_json::json!({
                    "key_code": shortcut.key_code,
                    "modifiers": shortcut.modifiers,
                    "kind": shortcut.kind,
                    "display": shortcut.display_string(),
                }));
            }
            Err(_) => break,
        }
    });
}

/// Spawns the spectrum emission timer (30fps) and monitors stream errors.
pub fn spawn_spectrum_emitter(
    app: AppHandle,
    state: Arc<AppState>,
    cmd_tx: crossbeam_channel::Sender<AudioCmd>,
    spectrum_data: Arc<std::sync::Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(SPECTRUM_INTERVAL_MS));
        let rt = state.runtime.lock().unwrap();
        let is_recording = rt.is_recording;
        let is_mic_testing = rt.mic_testing;
        drop(rt);

        if !is_recording && !is_mic_testing {
            continue;
        }

        // Detect audio stream error (e.g. device disconnected)
        if stream_error.load(Ordering::SeqCst) {
            log::warn!("Audio stream error detected (device disconnected?), forcing stop");
            state.runtime.lock().unwrap().is_recording = false;
            stream_error.store(false, Ordering::SeqCst);

            platform::play_sound("Basso");
            let _ = app.emit(crate::events::RECORDING_STOPPED, ());
            show_error_then_close(&app);
            continue;
        }

        let spectrum = spectrum_data.lock().unwrap().clone();
        let _ = cmd_tx.send(AudioCmd::GetSpectrum);
        let event_name = if is_mic_testing { crate::events::MIC_TEST_SPECTRUM } else { crate::events::SPECTRUM_DATA };
        let _ = app.emit(event_name, spectrum);
    });
}

/// Wrapper around audio command sender for mic test (managed by Tauri).
pub struct MicTestSender(pub crossbeam_channel::Sender<AudioCmd>);

pub fn new_recording_state(
    cmd_tx: crossbeam_channel::Sender<AudioCmd>,
    reply_rx: crossbeam_channel::Receiver<AudioReply>,
) -> RecordingState {
    RecordingState {
        key_down_time: None,
        last_short_tap_time: None,
        audio_tx: cmd_tx,
        audio_rx: reply_rx,
    }
}
