//! Live preview: transcribe the tail of the recording while the user is still
//! speaking, and show it in the subtitle strip.
//!
//! Whatever this produces is thrown away on release — the pasted text always
//! comes from the normal pipeline over the whole audio. That is what lets the
//! preview cut corners: a bounded window, a lighter engine, no post-processing.

use crate::audio::PreviewBuffer;
use crate::state::AppState;
use jona_engines::EngineCatalog;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

/// Gap between passes. A pass over the 15 s window costs ~0.83 s, so this leaves
/// the engine idle between refreshes rather than queueing work up.
const REFRESH_INTERVAL: Duration = Duration::from_millis(1_200);

/// Below this there is not enough speech to transcribe anything useful.
const MIN_SAMPLES: usize = 16_000;

/// What the strip shows. `settled` stays empty for now: it is where a periodic
/// full-audio pass will put text once that exists, leaving `tail` as the only
/// part still liable to change.
#[derive(Default)]
struct PreviewText {
    settled: String,
    tail: String,
}

impl PreviewText {
    fn render(&self) -> String {
        if self.settled.is_empty() {
            return self.tail.clone();
        }
        format!("{} {}", self.settled.trim_end(), self.tail.trim_start())
    }
}

/// Run preview passes until recording stops.
pub fn spawn(app: AppHandle, state: Arc<AppState>, buffer: Arc<PreviewBuffer>) {
    std::thread::spawn(move || {
        let (engine_id, model_id, gpu) = {
            let s = state.settings.lock().unwrap();
            let chosen = if s.live_preview_model_id.is_empty() {
                s.selected_model_id.clone()
            } else {
                s.live_preview_model_id.clone()
            };
            let gpu = s.gpu_mode;
            match EngineCatalog::global().model_by_id(&chosen) {
                Some(m) => (m.engine_id.clone(), chosen, gpu),
                None => {
                    log::warn!("Live preview: unknown model {chosen}, preview disabled");
                    return;
                }
            }
        };

        let scratch = std::env::temp_dir().join("jonawhisper-preview.wav");
        let mut text = PreviewText::default();

        while state.audio_flags.is_recording() {
            std::thread::sleep(REFRESH_INTERVAL);
            if !state.audio_flags.is_recording() {
                break;
            }

            let samples = buffer.snapshot();
            if samples.len() < MIN_SAMPLES {
                continue;
            }

            // Last check before taking the engine: preview and final transcription
            // share one context, so a pass started here would make the paste wait
            // for it. Cannot be eliminated — only kept as narrow as possible.
            if !state.audio_flags.is_recording() {
                break;
            }

            let started = std::time::Instant::now();
            match transcribe_tail(&state, &engine_id, &model_id, gpu, &scratch, &samples) {
                Ok(tail) if !tail.is_empty() => {
                    text.tail = tail;
                    crate::ui::subtitle::set_text(&app, &text.render());
                    log::trace!(
                        "Live preview: {:.1}s of audio in {:.2}s",
                        samples.len() as f32 / 16_000.0,
                        started.elapsed().as_secs_f32()
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    // A failed preview must never disturb the recording itself.
                    log::debug!("Live preview pass failed: {e}");
                }
            }
        }

        buffer.clear();
        let _ = std::fs::remove_file(&scratch);
    });
}

fn transcribe_tail(
    state: &AppState,
    engine_id: &str,
    model_id: &str,
    gpu: jona_types::GpuMode,
    scratch: &std::path::Path,
    samples: &[f32],
) -> Result<String, String> {
    super::pipeline::write_wav_f32(scratch, samples)?;

    let catalog = EngineCatalog::global();
    let engine = catalog
        .engine_by_id(engine_id)
        .ok_or_else(|| format!("engine {engine_id} not found"))?;
    let model = catalog
        .model_by_id(model_id)
        .ok_or_else(|| format!("model {model_id} not found"))?;
    let key = engine.context_key(&model, gpu);

    state
        .contexts
        .run_with(
            engine_id,
            &key,
            || engine.create_context(&model, gpu),
            |ctx| engine.transcribe(ctx, scratch, "auto"),
        )
        .map_err(|e| e.to_string())
        .map(|tr| tr.text.trim().to_string())
}

/// Recording stopped: drop the strip's contents so a stale line never outlives
/// the dictation that produced it.
pub fn reset(app: &AppHandle) {
    crate::ui::subtitle::set_text(app, "…");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_alone_renders_as_is() {
        let t = PreviewText { settled: String::new(), tail: "bonjour".into() };
        assert_eq!(t.render(), "bonjour");
    }

    #[test]
    fn settled_and_tail_join_with_one_space() {
        let t = PreviewText { settled: "bonjour ".into(), tail: " le monde".into() };
        assert_eq!(t.render(), "bonjour le monde");
    }

    #[test]
    fn window_holds_fifteen_seconds() {
        assert_eq!(crate::audio::PREVIEW_WINDOW_SAMPLES, 15 * 16_000);
    }
}
