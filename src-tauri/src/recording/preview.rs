//! Live preview: transcribe the recording while the user is still speaking, and
//! show it in the subtitle strip.
//!
//! Whatever this produces is thrown away on release — the pasted text always
//! comes from the normal pipeline over the whole audio. That is what lets the
//! preview cut corners: a lighter engine, no post-processing, an approximate
//! seam between its two halves.
//!
//! Two passes share the work:
//!
//! - the **tail** covers everything after a fixed cut point, refreshed every
//!   REFRESH_INTERVAL so the newest words appear quickly;
//! - the **settled** half covers the audio before that cut, recomputed only when
//!   the cut moves, which is what gives the older text full context.
//!
//! The cut is a sample offset, not a position in the text: splitting the audio
//! means the two halves never overlap and never leave a gap, which splitting the
//! text would require per-word timestamps to guarantee.

use crate::audio::PreviewBuffer;
use crate::state::AppState;
use jona_engines::EngineCatalog;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

const SAMPLE_RATE: usize = 16_000;

/// Gap between tail passes. A pass over the longest tail costs ~0.83 s, so this
/// leaves the engine idle between refreshes rather than queueing work up.
const REFRESH_INTERVAL: Duration = Duration::from_millis(1_200);

/// Below this there is not enough speech to transcribe anything useful.
const MIN_SAMPLES: usize = SAMPLE_RATE;

/// Longest the tail may get before the cut advances. Measured: 15 s transcribes
/// in 0.83 s and matches the full-audio text word for word; at 5 s the model
/// loses context and turns "sans voler" into "s'envoler".
const TAIL_MAX_SAMPLES: usize = 15 * SAMPLE_RATE;

/// Where the cut lands when it moves: the tail restarts this long, then grows
/// back towards TAIL_MAX_SAMPLES.
const TAIL_AFTER_CUT_SAMPLES: usize = 5 * SAMPLE_RATE;

/// The settled pass re-reads everything before the cut, so its cost grows with
/// the dictation. Past this the cut stops advancing and the tail goes back to
/// being a sliding window — older text scrolls away, but a two-minute dictation
/// never spends ten seconds blocking the engine.
const SETTLED_MAX_SAMPLES: usize = 60 * SAMPLE_RATE;

/// What the strip shows: text that will no longer change, plus the tail that
/// still might.
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
        if self.tail.is_empty() {
            return self.settled.clone();
        }
        format!("{} {}", self.settled.trim_end(), self.tail.trim_start())
    }
}

/// Run preview passes until recording stops.
pub fn spawn(app: AppHandle, state: Arc<AppState>, buffer: Arc<PreviewBuffer>) {
    std::thread::spawn(move || {
        let Some(engine) = resolve_engine(&state) else {
            return;
        };
        let scratch = std::env::temp_dir().join("jonawhisper-preview.wav");
        let mut text = PreviewText::default();
        let mut cut = 0usize;

        while state.audio_flags.is_recording() {
            std::thread::sleep(REFRESH_INTERVAL);
            if !state.audio_flags.is_recording() {
                break;
            }

            let total = buffer.len();
            if total.saturating_sub(cut) < MIN_SAMPLES {
                continue;
            }

            // Move the cut once the tail outgrows its window, unless the settled
            // half has reached the point where re-reading it costs too much.
            let new_cut = total.saturating_sub(TAIL_AFTER_CUT_SAMPLES);
            if total - cut > TAIL_MAX_SAMPLES && new_cut <= SETTLED_MAX_SAMPLES {
                if !state.audio_flags.is_recording() {
                    break;
                }
                match transcribe(&state, &engine, &scratch, &buffer.slice(0, new_cut)) {
                    Ok(settled) => {
                        text.settled = settled;
                        cut = new_cut;
                    }
                    Err(e) => log::debug!("Live preview: settled pass failed: {e}"),
                }
            }

            // Once the settled half is frozen, keep the tail bounded so its cost
            // stays flat: the oldest words scroll out, as they did before.
            let tail_from = if total - cut > TAIL_MAX_SAMPLES {
                total - TAIL_MAX_SAMPLES
            } else {
                cut
            };

            // Last check before taking the engine: preview and final transcription
            // share one context, so a pass started here would make the paste wait
            // for it. Cannot be eliminated — only kept as narrow as possible.
            if !state.audio_flags.is_recording() {
                break;
            }

            // A sign that something is listening. Until the first pass returns
            // words there is nothing to show, and an absent strip is
            // indistinguishable from a broken one — which is exactly how it
            // read. Shown here rather than at open() so it appears when there
            // is audio to work on, not while the user has yet to speak.
            if text.settled.is_empty() && text.tail.is_empty() {
                crate::ui::subtitle::set_text(&app, "…");
            }

            let started = std::time::Instant::now();
            match transcribe(&state, &engine, &scratch, &buffer.slice(tail_from, total)) {
                Ok(tail) if !tail.is_empty() => {
                    text.tail = tail;
                    crate::ui::subtitle::set_text(&app, &text.render());
                    log::trace!(
                        "Live preview: tail {:.1}s in {:.2}s (settled {:.1}s)",
                        (total - tail_from) as f32 / SAMPLE_RATE as f32,
                        started.elapsed().as_secs_f32(),
                        cut as f32 / SAMPLE_RATE as f32,
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    // A failed preview must never disturb the recording itself.
                    log::debug!("Live preview: tail pass failed: {e}");
                }
            }
        }

        buffer.clear();
        let _ = std::fs::remove_file(&scratch);
    });
}

/// Engine, model and GPU mode for the preview, resolved once.
struct PreviewEngine {
    engine_id: String,
    model_id: String,
    gpu: jona_types::GpuMode,
}

fn resolve_engine(state: &AppState) -> Option<PreviewEngine> {
    let s = state.settings.lock().unwrap();
    let chosen = if s.live_preview_model_id.is_empty() {
        s.selected_model_id.clone()
    } else {
        s.live_preview_model_id.clone()
    };
    match EngineCatalog::global().model_by_id(&chosen) {
        Some(m) => Some(PreviewEngine {
            engine_id: m.engine_id.clone(),
            model_id: chosen,
            gpu: s.gpu_mode,
        }),
        None => {
            log::warn!("Live preview: unknown model {chosen}, preview disabled");
            None
        }
    }
}

fn transcribe(
    state: &AppState,
    preview: &PreviewEngine,
    scratch: &std::path::Path,
    samples: &[f32],
) -> Result<String, String> {
    if samples.len() < MIN_SAMPLES {
        return Ok(String::new());
    }
    super::pipeline::write_wav_f32(scratch, samples)?;

    let catalog = EngineCatalog::global();
    let engine = catalog
        .engine_by_id(&preview.engine_id)
        .ok_or_else(|| format!("engine {} not found", preview.engine_id))?;
    let model = catalog
        .model_by_id(&preview.model_id)
        .ok_or_else(|| format!("model {} not found", preview.model_id))?;
    let key = engine.context_key(&model, preview.gpu);

    state
        .contexts
        .run_with(
            &preview.engine_id,
            &key,
            || engine.create_context(&model, preview.gpu),
            |ctx| engine.transcribe(ctx, scratch, "auto"),
        )
        .map_err(|e| e.to_string())
        .map(|tr| tr.text.trim().to_string())
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
    fn settled_alone_renders_as_is() {
        let t = PreviewText { settled: "bonjour".into(), tail: String::new() };
        assert_eq!(t.render(), "bonjour");
    }

    #[test]
    fn settled_and_tail_join_with_one_space() {
        let t = PreviewText { settled: "bonjour ".into(), tail: " le monde".into() };
        assert_eq!(t.render(), "bonjour le monde");
    }

    /// The tail must never exceed the window that was measured as safe, and the
    /// cut must leave enough behind for the next tail to have context.
    #[test]
    fn window_constants_stay_consistent() {
        assert!(TAIL_AFTER_CUT_SAMPLES < TAIL_MAX_SAMPLES);
        assert!(SETTLED_MAX_SAMPLES > TAIL_MAX_SAMPLES);
        assert!(TAIL_MAX_SAMPLES <= crate::audio::PREVIEW_MAX_SAMPLES);
    }
}
