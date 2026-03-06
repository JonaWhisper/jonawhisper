use crate::audio;
use crate::events;
use crate::platform::hotkey;
use crate::platform::paste;
use crate::platform;
use crate::asr;
use crate::cleanup;
use crate::state::AppState;
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
            state.audio_flags.set_mic_testing(false);
            let _ = rec.audio_tx.send(AudioCmd::StopMicTest);
            let _ = app.emit(crate::events::MIC_TEST_STOPPED, ());
        }
        rt.is_recording = true;
        state.audio_flags.set_recording(true);
        rt.transcription_cancelled = false;
    }
    rec.key_down_time = Some(Instant::now());
    PILL_CLOSE_GENERATION.fetch_add(1, Ordering::SeqCst);

    // Show pill immediately in Preparing mode (before stream starts)
    crate::ui::pill::open(app, crate::ui::pill::PillMode::Preparing);
    crate::ui::tray::set_tray_state(app, "recording");

    let (device_uid, ducking_enabled, ducking_level) = {
        let s = state.settings.lock().unwrap();
        (s.selected_input_device_uid.clone(), s.audio_ducking_enabled, s.audio_ducking_level)
    };
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    let _ = rec.audio_rx.recv();
    // Duck AFTER stream started: BT profile switch (A2DP→HFP) has already happened,
    // so we read/set volume in the actual audio state.
    if ducking_enabled {
        platform::audio_ducking::duck_volume(ducking_level);
    }

    // Stream is ready — transition to Recording mode + audible cue
    platform::play_sound("Tink");
    crate::ui::pill::set_mode(crate::ui::pill::PillMode::Recording);
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
        state.audio_flags.set_recording(false);
    }

    // Restore BEFORE stopping stream: on BT, the mic stream keeps HFP active,
    // so we restore volume in the same audio profile state as when we ducked.
    // Stopping the stream triggers HFP→A2DP switch which would swallow the restore.
    platform::audio_ducking::restore_volume();
    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
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
            crate::ui::tray::close_pill_window(app);
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
    // count = queue len after enqueue; +1 if already transcribing = total pending
    let is_transcribing = state.runtime.lock().unwrap().is_transcribing;
    crate::ui::pill::set_pending(count as u32 + if is_transcribing { 1 } else { 0 });
    crate::ui::pill::set_mode(crate::ui::pill::PillMode::Transcribing);
    crate::ui::tray::set_tray_state(app, "transcribing");

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
        crate::ui::tray::close_pill_window(app);
    } else {
        crate::ui::pill::set_mode(crate::ui::pill::PillMode::Transcribing);
        crate::ui::tray::set_tray_state(app, "transcribing");
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

        let qc = state.queue_count();
        let _ = app.emit(
            crate::events::TRANSCRIPTION_STARTED,
            serde_json::json!({ "queue_count": qc }),
        );
        // pending = items still in queue + the one we're about to process
        crate::ui::pill::set_pending(qc as u32 + 1);

        // VAD pre-check: discard silence, trim edges
        let vad_enabled = state.settings.lock().unwrap().vad_enabled;
        let mut vad_trimmed = false;
        if vad_enabled {
            let path_clone = audio_path.clone();
            let vad_result = tokio::task::spawn_blocking(move || {
                vad_preprocess(&path_clone)
            }).await;

            match vad_result {
                Ok(VadResult::NoSpeech) => {
                    log::info!("VAD: no speech detected, discarding");
                    platform::play_sound("Basso");
                    let _ = std::fs::remove_file(&audio_path);
                    state.runtime.lock().unwrap().is_transcribing = false;
                    // If queue still has items, continue processing them
                    if state.queue_count() > 0 {
                        continue;
                    }
                    show_error_then_close(app);
                    return;
                }
                Ok(VadResult::Trimmed) => {
                    log::info!("VAD: trimmed silence from audio");
                    vad_trimmed = true;
                }
                Ok(VadResult::Unchanged) => {}
                Err(e) => {
                    log::warn!("VAD task error, proceeding with original audio: {}", e);
                }
            }
        }

        let had_error = run_transcription(app, state, &audio_path, vad_trimmed).await;
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

    let (should_close, had_content) = {
        let mut rt = state.runtime.lock().unwrap();
        if !rt.is_recording {
            let hc = rt.last_paste_had_content;
            rt.last_paste_had_content = false;
            (true, hc)
        } else {
            (false, false)
        }
    };
    if should_close {
        if had_content {
            // Show success checkmark briefly before closing
            crate::ui::pill::set_mode(crate::ui::pill::PillMode::Success);
            let gen = PILL_CLOSE_GENERATION.load(Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(600)).await;
            // Abort if a new recording started during the sleep
            if PILL_CLOSE_GENERATION.load(Ordering::SeqCst) != gen {
                return;
            }
        }
        crate::ui::tray::close_pill_window(app);
    }
}

async fn run_transcription(
    app: &AppHandle,
    state: &Arc<AppState>,
    audio_path: &std::path::Path,
    vad_trimmed: bool,
) -> bool {
    let state_clone = Arc::clone(state);
    let path = audio_path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        asr::transcribe(&state_clone, &path)
    })
    .await;

    match result {
        Ok(Ok(text)) => {
            if state.runtime.lock().unwrap().transcription_cancelled {
                log::info!("Transcription result discarded (cancelled)");
                return false;
            }
            handle_transcription_result(app, state, &text, vad_trimmed).await;
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

async fn handle_transcription_result(app: &AppHandle, state: &Arc<AppState>, text: &str, vad_trimmed: bool) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    // Read settings once
    let (model_id, lang, hall_filter, text_cleanup_enabled, cleanup_model_id,
         llm_model, llm_max_tokens, providers) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_model_id.clone(),
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
        let opts = cleanup::post_processor::PostProcessOptions {
            hallucination_filter: hall_filter,
        };
        cleanup::post_processor::preprocess(trimmed, &lang, &opts)
    };

    if processed.trim().is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    // Step 2: cleanup based on selected model
    let mut effective_cleanup_model_id = String::new();
    let cleanup_kind = if text_cleanup_enabled {
        crate::engines::CleanupKind::from_model_id(&cleanup_model_id)
    } else {
        crate::engines::CleanupKind::None
    };

    match cleanup_kind {
        crate::engines::CleanupKind::Correction => {
            // T5 correction (encoder-decoder) — finalize after
            let state_clone = Arc::clone(state);
            let text_for_correction = processed.clone();
            let model_id = cleanup_model_id.clone();
            match tokio::task::spawn_blocking(move || {
                run_t5_correction(&state_clone, &text_for_correction, &model_id)
            }).await {
                Ok(Ok(corrected)) => {
                    log::info!("T5 correction: {} → {}", processed.len(), corrected.len());
                    processed = corrected;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Ok(Err(e)) => log::warn!("T5 correction failed, using preprocessed result: {}", e),
                Err(e) => log::warn!("T5 correction task panicked: {}", e),
            }
            processed = cleanup::post_processor::finalize(&processed);
        }
        crate::engines::CleanupKind::Punctuation(runtime) => {
            // Punctuation restoration (BERT/Candle/PCS) — finalize after
            let state_clone = Arc::clone(state);
            let text_for_punct = processed.clone();
            let model_id = cleanup_model_id.clone();
            let engine_name = match runtime {
                crate::engines::PunctRuntime::Pcs => "PCS",
                crate::engines::PunctRuntime::BertCandle => "Candle",
                crate::engines::PunctRuntime::BertOrt => "BERT",
            };
            let punct_result = match runtime {
                crate::engines::PunctRuntime::Pcs => {
                    tokio::task::spawn_blocking(move || {
                        run_pcs_punctuation(&state_clone, &text_for_punct, &model_id)
                    }).await
                }
                crate::engines::PunctRuntime::BertCandle => {
                    tokio::task::spawn_blocking(move || {
                        run_candle_punctuation(&state_clone, &text_for_punct, &model_id)
                    }).await
                }
                crate::engines::PunctRuntime::BertOrt => {
                    tokio::task::spawn_blocking(move || {
                        run_bert_punctuation(&state_clone, &text_for_punct, &model_id)
                    }).await
                }
            };
            match punct_result {
                Ok(Ok(punctuated)) => {
                    log::info!("{} punctuation: {} → {}", engine_name, processed.len(), punctuated.len());
                    processed = punctuated;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Ok(Err(e)) => log::warn!("{} punctuation failed, using preprocessed result: {}", engine_name, e),
                Err(e) => log::warn!("{} punctuation task panicked: {}", engine_name, e),
            }
            processed = cleanup::post_processor::finalize(&processed);
        }
        crate::engines::CleanupKind::LocalLlm => {
            // Local LLM cleanup — finalize before
            processed = cleanup::post_processor::finalize(&processed);
            let effective_max_tokens = effective_llm_tokens(processed.len(), llm_max_tokens);
            let state_clone = Arc::clone(state);
            let text_for_llm = processed.clone();
            let lang_for_llm = lang.clone();
            let model_id = cleanup_model_id.clone();
            let llm_result = match tokio::task::spawn_blocking(move || {
                run_local_llm_cleanup(&state_clone, &text_for_llm, &lang_for_llm, &model_id, effective_max_tokens)
            }).await {
                Ok(result) => result,
                Err(e) => Err(crate::cleanup::LlmError::Inference(format!("LLM task panicked: {}", e))),
            };
            match llm_result {
                Ok(cleaned) => {
                    log::info!("LLM cleanup: {} → {}", processed.len(), cleaned.len());
                    processed = cleaned;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Err(e) => log::warn!("LLM cleanup failed (fallback to raw): {}", e),
            }
        }
        crate::engines::CleanupKind::CloudLlm(provider_id) => {
            // Cloud LLM cleanup — finalize before
            processed = cleanup::post_processor::finalize(&processed);
            let effective_max_tokens = effective_llm_tokens(processed.len(), llm_max_tokens);
            let llm_result = if let Some(provider) = providers.iter().find(|p| p.id == provider_id) {
                if !provider.has_llm() {
                    log::warn!("Cloud provider '{}' does not support LLM", provider.name);
                    Err(crate::cleanup::LlmError::NotConfigured)
                } else {
                    crate::cleanup::llm_cloud::cleanup_text(&processed, &lang, provider, &llm_model, effective_max_tokens).await
                }
            } else {
                log::warn!("Cloud LLM provider '{}' not found", provider_id);
                Err(crate::cleanup::LlmError::NotConfigured)
            };
            match llm_result {
                Ok(cleaned) => {
                    log::info!("LLM cleanup: {} → {}", processed.len(), cleaned.len());
                    processed = cleaned;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                }
                Err(e) => log::warn!("LLM cleanup failed (fallback to raw): {}", e),
            }
        }
        crate::engines::CleanupKind::None => {
            processed = cleanup::post_processor::finalize(&processed);
        }
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
    state.add_history(processed.clone(), model_id, lang.clone(), effective_cleanup_model_id.clone(), hall_filter, vad_trimmed);
    platform::play_sound("Glass");

    let _ = app.emit(
        crate::events::TRANSCRIPTION_COMPLETE,
        serde_json::json!({
            "text": processed,
            "cleanup_model_id": effective_cleanup_model_id,
            "hallucination_filter": hall_filter,
            "vad_trimmed": vad_trimmed,
        }),
    );
}

fn effective_llm_tokens(text_len: usize, max: u32) -> u32 {
    std::cmp::min(max, std::cmp::max((text_len as u32) / 3 + 64, 128))
}

// -- Cleanup model runners --

fn run_bert_punctuation(
    state: &Arc<AppState>,
    text: &str,
    model_id: &str,
) -> Result<String, String> {
    // Auto-select first downloaded BERT model if ID is empty
    let (_, model_path) = if model_id.is_empty() {
        let catalog = crate::engines::EngineCatalog::global();
        let model = catalog.all_models().into_iter()
            .find(|m| m.engine_id == "bert-punctuation" && m.is_downloaded())
            .ok_or_else(|| "No punctuation model downloaded".to_string())?;
        let path = model.local_path();
        (model, path)
    } else {
        crate::engines::resolve_model(model_id)?
    };
    let mut guard = state.inference.cleanup.bert.get_or_load(model_id, || {
        crate::cleanup::bert::BertContext::load(&model_path, model_id)
    })?;
    crate::cleanup::bert::restore_punctuation(guard.as_mut().unwrap(), text)
}

fn run_candle_punctuation(
    state: &Arc<AppState>,
    text: &str,
    model_id: &str,
) -> Result<String, String> {
    let (_, model_path) = crate::engines::resolve_model(model_id)?;
    let guard = state.inference.cleanup.candle_punct.get_or_load(model_id, || {
        crate::cleanup::candle::CandlePunctContext::load(&model_path, model_id)
    })?;
    crate::cleanup::candle::restore_punctuation(guard.as_ref().unwrap(), text)
}

fn run_pcs_punctuation(
    state: &Arc<AppState>,
    text: &str,
    model_id: &str,
) -> Result<String, String> {
    let (_, model_path) = crate::engines::resolve_model(model_id)?;
    let mut guard = state.inference.cleanup.pcs.get_or_load(model_id, || {
        crate::cleanup::pcs::PcsContext::load(&model_path, model_id)
    })?;
    crate::cleanup::pcs::restore_punctuation_and_case(guard.as_mut().unwrap(), text)
}

fn run_t5_correction(
    state: &Arc<AppState>,
    text: &str,
    model_id: &str,
) -> Result<String, String> {
    let (_, model_dir) = crate::engines::resolve_model(model_id)?;
    let mut guard = state.inference.cleanup.t5.get_or_load(model_id, || {
        crate::cleanup::t5::T5Context::load(&model_dir, model_id)
    })?;
    crate::cleanup::t5::correct(guard.as_mut().unwrap(), text)
}

fn run_local_llm_cleanup(
    state: &Arc<AppState>,
    text: &str,
    language: &str,
    model_id: &str,
    max_tokens: u32,
) -> Result<String, crate::cleanup::LlmError> {
    use crate::cleanup::LlmError;
    if model_id.is_empty() {
        return Err(LlmError::NotConfigured);
    }
    let (_, model_path) = crate::engines::resolve_model(model_id)
        .map_err(|e| LlmError::Inference(e))?;
    let guard = state.inference.cleanup.llm.get_or_load(model_id, || {
        crate::cleanup::llm_local::LlmContext::load(&model_path, model_id)
    })?;
    crate::cleanup::llm_local::cleanup_text(guard.as_ref().unwrap(), text, language, max_tokens as usize)
}

// -- VAD helpers --

enum VadResult {
    NoSpeech,
    Trimmed,
    Unchanged,
}

fn vad_preprocess(audio_path: &std::path::Path) -> VadResult {
    let audio = match crate::audio::read_wav_f32(audio_path) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("VAD: failed to read WAV, skipping: {}", e);
            return VadResult::Unchanged;
        }
    };

    match crate::cleanup::vad::analyze(&audio) {
        crate::cleanup::vad::VadAnalysis::NoSpeech => VadResult::NoSpeech,
        crate::cleanup::vad::VadAnalysis::Speech { start, end } => {
            if start == 0 && end == audio.len() {
                return VadResult::Unchanged;
            }
            let trimmed = &audio[start..end];
            if let Err(e) = write_wav_f32(audio_path, trimmed) {
                log::warn!("VAD: failed to write trimmed WAV, using original: {}", e);
                return VadResult::Unchanged;
            }
            VadResult::Trimmed
        }
    }
}

fn write_wav_f32(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV: {e}"))?;
    for &s in samples {
        writer.write_sample(s).map_err(|e| format!("WAV write error: {e}"))?;
    }
    writer.finalize().map_err(|e| format!("WAV finalize error: {e}"))?;
    Ok(())
}

// -- Cleanup helpers --

fn show_error_then_close(app: &AppHandle) {
    crate::ui::pill::set_mode(crate::ui::pill::PillMode::Error);
    let gen = PILL_CLOSE_GENERATION.load(Ordering::SeqCst);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ERROR_DISPLAY_MS)).await;
        // Only close if no new recording started since the error
        if PILL_CLOSE_GENERATION.load(Ordering::SeqCst) == gen {
            crate::ui::tray::close_pill_window(&app_clone);
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
        state.audio_flags.set_recording(false);
    }

    // Restore BEFORE stopping stream (same rationale as stop_recording_and_enqueue)
    platform::audio_ducking::restore_volume();
    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
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
    crate::ui::pill::set_pending(0);
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
            if (name.starts_with("jona_whisper_") || name.starts_with("whisper_dictate_")) && name.ends_with(".wav") {
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
                let mode = state.settings.lock().unwrap().recording_mode;
                let is_recording = state.runtime.lock().unwrap().is_recording;
                let mut rec = rec_state.lock().unwrap();
                if mode == crate::state::RecordingMode::Toggle && is_recording {
                    stop_recording_and_enqueue(&app, &state, &mut rec);
                } else {
                    start_recording(&app, &state, &mut rec);
                }
            }
            Ok(hotkey::HotkeyEvent::KeyUp) => {
                let mode = state.settings.lock().unwrap().recording_mode;
                if mode != crate::state::RecordingMode::Toggle {
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
            Ok(hotkey::HotkeyEvent::CaptureUpdate { modifiers, key_codes }) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_UPDATE, serde_json::json!({
                    "modifiers": modifiers,
                    "key_codes": key_codes,
                }));
            }
            Ok(hotkey::HotkeyEvent::CaptureComplete(shortcut)) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_COMPLETE, serde_json::json!({
                    "key_codes": shortcut.key_codes,
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

        // Fast lock-free check — avoids mutex contention when idle
        if !state.audio_flags.is_active() {
            continue;
        }

        let is_mic_testing = state.audio_flags.is_mic_testing();

        // Detect audio stream error (e.g. device disconnected)
        if stream_error.load(Ordering::SeqCst) {
            log::warn!("Audio stream error detected (device disconnected?), forcing stop");
            state.runtime.lock().unwrap().is_recording = false;
            state.audio_flags.set_recording(false);
            stream_error.store(false, Ordering::SeqCst);

            // Actually stop the cpal stream — without this the mic stays active
            let _ = cmd_tx.send(AudioCmd::StopRecording);

            platform::play_sound("Basso");
            let _ = app.emit(crate::events::RECORDING_STOPPED, ());
            show_error_then_close(&app);
            continue;
        }

        let spectrum = spectrum_data.lock().unwrap().clone();
        let _ = cmd_tx.send(AudioCmd::GetSpectrum);
        if is_mic_testing {
            let _ = app.emit(crate::events::MIC_TEST_SPECTRUM, &spectrum);
        } else {
            // Feed spectrum directly to native pill (no Tauri event needed)
            crate::ui::pill::set_spectrum(&spectrum);
        }
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
