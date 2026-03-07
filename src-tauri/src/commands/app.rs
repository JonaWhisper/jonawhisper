use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn get_app_state(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    state.to_frontend_json()
}

#[tauri::command]
pub fn start_shortcut_capture(capture: tauri::State<'_, Arc<crate::platform::hotkey::CaptureControl>>) {
    capture.enter();
}

#[tauri::command]
pub fn stop_shortcut_capture(capture: tauri::State<'_, Arc<crate::platform::hotkey::CaptureControl>>) {
    capture.exit();
}

#[tauri::command]
pub async fn simulate_pill_test(app: AppHandle, _count: Option<u32>) {
    use crate::ui::pill::{self, PillMode};
    use std::time::Duration;

    fn fake_spectrum(frame: u32) -> Vec<f32> {
        (0..12)
            .map(|i| {
                let phase = (frame as f32 * 0.15) + (i as f32 * 0.5);
                (phase.sin() * 0.5 + 0.5) * 0.8
            })
            .collect()
    }

    async fn recording_phase(app: &AppHandle, secs: f32) {
        pill::set_mode(PillMode::Recording);
        let _ = app.emit(crate::events::RECORDING_STARTED, ());
        let frames = (secs * 30.0) as u32;
        for frame in 0..frames {
            pill::set_spectrum(&fake_spectrum(frame));
            tokio::time::sleep(Duration::from_millis(33)).await;
        }
    }

    log::info!("=== Pill test: full flow ===");

    // ── 1. Simple recording → transcribing → success ──
    log::info!("[1/5] Single recording → success");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    recording_phase(&app, 2.0).await;

    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "Test single recording" }));
    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 2. Recording → transcribing → error ──
    log::info!("[2/5] Single recording → error");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    recording_phase(&app, 1.5).await;

    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(1500)).await;

    pill::set_mode(PillMode::Error);
    tokio::time::sleep(Duration::from_millis(1000)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 3. Queue: record while transcribing (2 items queued) ──
    log::info!("[3/5] Queue: record during transcription (pending=2 then 3)");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // First recording
    recording_phase(&app, 1.5).await;
    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Second recording while first is transcribing
    pill::set_mode(PillMode::Recording);
    recording_phase(&app, 1.0).await;
    pill::set_pending(2);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 2 }));
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Third recording while queue has 2
    pill::set_mode(PillMode::Recording);
    recording_phase(&app, 1.0).await;
    pill::set_pending(3);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 3 }));

    // Process queue: 3 → 2 → 1 → done
    for remaining in (0..3).rev() {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        pill::set_pending(remaining + 1);
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": format!("Queue item {}", 3 - remaining) }));
        if remaining > 0 {
            pill::set_pending(remaining);
            let _ = app.emit(crate::events::TRANSCRIPTION_STARTED, serde_json::json!({ "queue_count": remaining }));
        }
    }

    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 4. Preparing mode (model loading) ──
    log::info!("[4/5] Preparing mode");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;
    pill::set_mode(PillMode::Preparing);
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Transition to recording after model loaded
    recording_phase(&app, 1.5).await;
    pill::set_pending(1);
    pill::set_mode(PillMode::Transcribing);
    let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": 1 }));
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": "After preparing" }));
    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ── 5. Rapid fire: quick record → immediate re-record ──
    log::info!("[5/5] Rapid fire: 3 quick recordings back-to-back");
    crate::ui::tray::open_pill_window(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;

    for i in 1..=3u32 {
        recording_phase(&app, 0.5).await;
        pill::set_pending(i);
        pill::set_mode(PillMode::Transcribing);
        let _ = app.emit(crate::events::RECORDING_STOPPED, serde_json::json!({ "queue_count": i }));
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Drain queue
    for remaining in (0..3).rev() {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let _ = app.emit(crate::events::TRANSCRIPTION_COMPLETE, serde_json::json!({ "text": format!("Rapid {}", 3 - remaining) }));
        if remaining > 0 {
            pill::set_pending(remaining as u32);
            let _ = app.emit(crate::events::TRANSCRIPTION_STARTED, serde_json::json!({ "queue_count": remaining }));
        }
    }

    pill::set_mode(PillMode::Success);
    tokio::time::sleep(Duration::from_millis(800)).await;
    crate::ui::tray::close_pill_window(&app);

    log::info!("=== Pill test complete ===");
}
