use super::{show_error_then_close, PILL_CLOSE_GENERATION};
use crate::cleanup;
use crate::events;
use crate::platform;
use crate::platform::paste;
use crate::state::{AppState, HistoryEntry};
use jona_engines::{EngineCatalog, EngineError};
use jona_types::{TranscriptionResult, WordConfidence};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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
            events::TRANSCRIPTION_STARTED,
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
            let gen = PILL_CLOSE_GENERATION.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(600)).await;
            // Abort if a new recording started during the sleep
            if PILL_CLOSE_GENERATION.load(Ordering::Relaxed) != gen {
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
    let t0 = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        transcribe(&state_clone, &path)
    })
    .await;
    log::info!("Transcription total: {:.1}s", t0.elapsed().as_secs_f64());

    match result {
        Ok(Ok(tr)) => {
            if state.runtime.lock().unwrap().transcription_cancelled {
                log::info!("Transcription result discarded (cancelled)");
                return false;
            }
            handle_transcription_result(app, state, &tr.text, &tr.word_confidences, vad_trimmed).await;
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

async fn handle_transcription_result(app: &AppHandle, state: &Arc<AppState>, text: &str, word_confidences: &[WordConfidence], vad_trimmed: bool) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    // Pipeline step snapshots: capture text after each processing stage
    let mut pipeline_steps: Vec<(&str, String)> = Vec::new();
    pipeline_steps.push(("asr", trimmed.to_string()));

    // Log and serialize word confidence scores
    let word_scores_json = if !word_confidences.is_empty() {
        let scores: Vec<(&str, f32)> = word_confidences.iter()
            .filter_map(|wc| wc.confidence.map(|c| (wc.word.as_str(), c)))
            .collect();
        if !scores.is_empty() {
            log::info!("Confidence scores: {}", scores.iter()
                .map(|(w, c)| format!("{}={:.2}", w, c))
                .collect::<Vec<_>>().join(" "));
        }
        serde_json::to_string(&word_confidences.iter()
            .map(|wc| (wc.word.as_str(), wc.confidence.unwrap_or(-1.0)))
            .collect::<Vec<_>>()
        ).unwrap_or_default()
    } else {
        String::new()
    };

    // Read settings once
    let (model_id, lang, hall_filter, disfluency_removal, itn_enabled,
         spellcheck_enabled, punctuation_model_id, text_cleanup_enabled,
         cleanup_model_id, llm_model, llm_max_tokens, cleanup_provider) = {
        let s = state.settings.lock().unwrap();
        let provider = s.cleanup_model_id.strip_prefix("cloud:")
            .and_then(|pid| s.providers.iter().find(|p| p.id == pid).cloned());
        (
            s.selected_model_id.clone(),
            s.selected_language.clone(),
            s.hallucination_filter_enabled,
            s.disfluency_removal_enabled,
            s.itn_enabled,
            s.spellcheck_enabled,
            s.punctuation_model_id.clone(),
            s.text_cleanup_enabled,
            s.cleanup_model_id.clone(),
            s.llm_model.clone(),
            s.llm_max_tokens,
            provider,
        )
    };

    // Step 1: preprocess (hallucination filter + dictation commands + disfluency removal)
    let mut processed = {
        let opts = cleanup::post_processor::PostProcessOptions {
            hallucination_filter: hall_filter,
            disfluency_removal,
        };
        cleanup::post_processor::preprocess(trimmed, &lang, &opts)
    };
    if processed != trimmed {
        pipeline_steps.push(("preprocess", processed.clone()));
    }

    if processed.trim().is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit(events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "" }));
        return;
    }

    let mut effective_cleanup_model_id = String::new();
    let gpu = state.settings.lock().unwrap().gpu_mode;

    // Step 2a: punctuation (PCS/BERT) — runs first if configured
    if !punctuation_model_id.is_empty() {
        match run_local_engine(state, &punctuation_model_id, &processed, &lang, gpu, 0).await {
            Ok(cleaned) => {
                log::info!("Punctuation: {} → {}", processed.len(), cleaned.len());
                if cleaned != processed {
                    processed = cleaned;
                    pipeline_steps.push(("punctuation", processed.clone()));
                } else {
                    processed = cleaned;
                }
            }
            Err(e) => log::warn!("Punctuation failed, continuing: {}", e),
        }
    }

    // Step 2b: spell-check (SymSpell) — runs after punctuation, before correction
    if spellcheck_enabled {
        let before = processed.clone();
        processed = cleanup::symspell_correct::auto_correct(&processed, &lang, word_confidences);
        if processed != before {
            log::debug!("SymSpell: «{}» → «{}»", before, processed);
            pipeline_steps.push(("spellcheck", processed.clone()));
        }
    }

    // Step 2c: cleanup (correction/LLM) — runs after spell-check
    let has_cleanup = text_cleanup_enabled && !cleanup_model_id.is_empty();
    let mut finalized = false;

    if has_cleanup {
        // Cloud LLM: special async path (provider-based, not an engine crate)
        if let Some(provider_id) = cleanup_model_id.strip_prefix("cloud:") {
            let before_finalize = processed.clone();
            processed = cleanup::post_processor::finalize(&processed);
            finalized = true;
            if processed != before_finalize {
                pipeline_steps.push(("finalize", processed.clone()));
            }
            let effective_max_tokens = effective_llm_tokens(processed.len(), llm_max_tokens);
            let llm_result = if let Some(ref provider) = cleanup_provider {
                if !provider.has_llm() {
                    log::warn!("Cloud provider '{}' does not support LLM", provider.name);
                    Err(cleanup::LlmError::NotConfigured)
                } else {
                    cleanup::llm_cloud::cleanup_text(&processed, &lang, provider, &llm_model, effective_max_tokens).await
                }
            } else {
                log::warn!("Cloud LLM provider '{}' not found", provider_id);
                Err(cleanup::LlmError::NotConfigured)
            };
            match llm_result {
                Ok(cleaned) => {
                    log::info!("Cloud LLM cleanup: {} → {}", processed.len(), cleaned.len());
                    let changed = cleaned != processed;
                    processed = cleaned;
                    effective_cleanup_model_id = cleanup_model_id.clone();
                    if changed {
                        pipeline_steps.push(("correction", processed.clone()));
                    }
                }
                Err(e) => log::warn!("Cloud LLM cleanup failed (fallback to raw): {}", e),
            }
        } else {
            // Local engine cleanup — dynamic dispatch via ASREngine trait
            let catalog = EngineCatalog::global();
            if let Some(model) = catalog.model_by_id(&cleanup_model_id) {
                if let Some(engine) = catalog.engine_by_id(&model.engine_id) {
                    let finalize_before = engine.finalize_before_cleanup();
                    if finalize_before {
                        let before_finalize = processed.clone();
                        processed = cleanup::post_processor::finalize(&processed);
                        finalized = true;
                        if processed != before_finalize {
                            pipeline_steps.push(("finalize", processed.clone()));
                        }
                    }

                    let max_tok = if finalize_before {
                        effective_llm_tokens(processed.len(), llm_max_tokens) as usize
                    } else {
                        0
                    };

                    match run_local_engine(state, &cleanup_model_id, &processed, &lang, gpu, max_tok).await {
                        Ok(cleaned) => {
                            log::info!("{} cleanup: {} → {}", model.engine_id, processed.len(), cleaned.len());
                            let changed = cleaned != processed;
                            processed = cleaned;
                            effective_cleanup_model_id = cleanup_model_id.clone();
                            if changed {
                                pipeline_steps.push(("correction", processed.clone()));
                            }
                        }
                        Err(e) => log::warn!("{} cleanup failed, using preprocessed result: {}", model.engine_id, e),
                    }
                } else {
                    log::warn!("Unknown cleanup engine for model: {}", cleanup_model_id);
                }
            } else {
                log::warn!("Cleanup model not found: {}", cleanup_model_id);
            }
        }
    }

    if !finalized {
        let before = processed.clone();
        processed = cleanup::post_processor::finalize(&processed);
        if processed != before {
            pipeline_steps.push(("finalize", processed.clone()));
        }
    }

    // Step 3: ITN (Inverse Text Normalization) — numbers, ordinals, currencies, units
    if itn_enabled {
        let before = processed.clone();
        processed = cleanup::itn::apply_itn(&processed, &lang);
        if processed != before {
            pipeline_steps.push(("itn", processed.clone()));
        }
    }

    // Serialize pipeline steps as raw_text (JSON array of [step, text] tuples)
    let raw_text = if pipeline_steps.len() > 1 {
        serde_json::to_string(&pipeline_steps).unwrap_or_default()
    } else {
        String::new()
    };

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
    state.add_history(HistoryEntry {
        text: processed.clone(),
        timestamp: 0, // filled by add_history
        model_id: model_id.clone(),
        language: lang.clone(),
        cleanup_model_id: effective_cleanup_model_id.clone(),
        hallucination_filter: hall_filter,
        vad_trimmed,
        punctuation_model_id: punctuation_model_id.clone(),
        spellcheck: spellcheck_enabled,
        disfluency_removal,
        itn: itn_enabled,
        raw_text: raw_text.clone(),
        word_scores: word_scores_json.clone(),
    });
    platform::play_sound("Glass");

    let _ = app.emit(
        events::TRANSCRIPTION_COMPLETE,
        serde_json::json!({
            "text": processed,
            "cleanup_model_id": effective_cleanup_model_id,
            "hallucination_filter": hall_filter,
            "vad_trimmed": vad_trimmed,
            "punctuation_model_id": punctuation_model_id,
            "spellcheck": spellcheck_enabled,
            "disfluency_removal": disfluency_removal,
            "itn": itn_enabled,
            "raw_text": raw_text,
            "word_scores": word_scores_json,
        }),
    );
}

/// Run a local engine cleanup via dynamic dispatch (spawn_blocking).
async fn run_local_engine(
    state: &Arc<AppState>,
    model_id: &str,
    text: &str,
    lang: &str,
    gpu: crate::state::GpuMode,
    max_tokens: usize,
) -> Result<String, String> {
    let catalog = EngineCatalog::global();
    let model = catalog.model_by_id(model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;
    if catalog.engine_by_id(&model.engine_id).is_none() {
        return Err(format!("Engine not found: {}", model.engine_id));
    }

    let state_clone = Arc::clone(state);
    let text_owned = text.to_string();
    let lang_owned = lang.to_string();
    let mid = model_id.to_string();
    let eid = model.engine_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        let catalog = EngineCatalog::global();
        let engine = catalog.engine_by_id(&eid).unwrap();
        let model = catalog.model_by_id(&mid).unwrap();
        let context_key = engine.context_key(&model, gpu);
        state_clone.contexts.run_with(
            &eid,
            &context_key,
            || engine.create_context(&model, gpu),
            |ctx| engine.cleanup(ctx, &text_owned, &lang_owned, max_tokens),
        )
    }).await;

    match result {
        Ok(Ok(cleaned)) => Ok(cleaned),
        Ok(Err(e)) => Err(e.to_string()),
        Err(e) => Err(format!("Task panicked: {}", e)),
    }
}

fn effective_llm_tokens(text_len: usize, max: u32) -> u32 {
    std::cmp::min(max, std::cmp::max((text_len as u32) / 3 + 64, 128))
}

// -- ASR dispatch --

fn transcribe(
    state: &AppState,
    audio_path: &std::path::Path,
) -> Result<TranscriptionResult, EngineError> {
    let (model_id, language, gpu_mode, asr_cloud_model, asr_provider) = {
        let s = state.settings.lock().unwrap();
        let provider = s.selected_model_id.strip_prefix("cloud:")
            .and_then(|pid| s.providers.iter().find(|p| p.id == pid).cloned());
        (
            s.selected_model_id.clone(),
            s.selected_language.clone(),
            s.gpu_mode,
            s.asr_cloud_model.clone(),
            provider,
        )
    };

    // Cloud dispatch: selected_model_id = "cloud:<provider_id>"
    if model_id.starts_with("cloud:") {
        let provider = asr_provider.ok_or_else(|| EngineError::ApiError(
                format!("ASR provider not found for '{}'", model_id)
            ))?;
        if !provider.has_asr() {
            return Err(EngineError::ApiError(
                format!("Provider '{}' does not support ASR transcription", provider.name)
            ));
        }
        return jona_provider::backend(provider.kind)
            .transcribe(&provider, &asr_cloud_model, audio_path, &language)
            .map_err(|e| EngineError::ApiError(e.to_string()));
    }

    // Local engine dispatch — fully dynamic via ASREngine trait
    let catalog = EngineCatalog::global();

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    let engine = catalog.engine_by_id(&model.engine_id)
        .ok_or_else(|| EngineError::LaunchFailed(format!("Unknown engine: {}", model.engine_id)))?;

    let context_key = engine.context_key(&model, gpu_mode);

    state.contexts.run_with(
        &model.engine_id,
        &context_key,
        || engine.create_context(&model, gpu_mode),
        |ctx| engine.transcribe(ctx, audio_path, &language),
    )
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
