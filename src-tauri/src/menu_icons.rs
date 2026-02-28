use crate::platform::audio_devices::AudioTransportType;
use std::sync::LazyLock;
use tauri::image::Image;

// Menu icon size: 16×16 RGBA (template icon for menu items)
const MENU_ICON_SIZE: u32 = 16;

// -- Shared SDF primitives (used by tray.rs too) --

/// Anti-alias a signed distance value: 1.0 inside, 0.0 outside, smooth at boundary.
pub(crate) fn sdf_aa(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Signed distance to a rounded rectangle centered at (cx, cy) with half-extents (hw, hh) and corner radius r.
pub(crate) fn sdf_rrect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - r).max(0.0);
    let qy = (py - cy).abs() - (hh - r).max(0.0);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

/// Signed distance to a circle centered at (cx, cy) with radius r.
pub(crate) fn sdf_circle(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r
}

/// Point-in-triangle test (barycentric sign check).
#[allow(clippy::too_many_arguments)]
pub(crate) fn point_in_triangle(
    px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32,
) -> bool {
    let d1 = (px - x2) * (y1 - y2) - (x1 - x2) * (py - y2);
    let d2 = (px - x3) * (y2 - y3) - (x2 - x3) * (py - y3);
    let d3 = (px - x1) * (y3 - y1) - (x3 - x1) * (py - y1);
    !(d1 < 0.0 && (d2 > 0.0 || d3 > 0.0)) && !(d1 > 0.0 && (d2 < 0.0 || d3 < 0.0))
}

/// Distance from point (px, py) to line segment (ax, ay)-(bx, by).
pub(crate) fn sdf_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let nx = ax + t * dx;
    let ny = ay + t * dy;
    ((px - nx).powi(2) + (py - ny).powi(2)).sqrt()
}

// -- Cached menu icons --

static MENU_ICONS: LazyLock<[Vec<u8>; 8]> = LazyLock::new(|| {
    [
        render_laptop(),
        render_usb(),
        render_bluetooth(),
        render_waves(),
        render_hard_drive(),
        render_zap(),
        render_monitor(),
        render_mic(),
    ]
});

/// Get a cached 16×16 template icon for an audio transport type.
pub fn transport_icon(t: &AudioTransportType) -> Image<'static> {
    let idx = match t {
        AudioTransportType::BuiltIn => 0,
        AudioTransportType::USB => 1,
        AudioTransportType::Bluetooth => 2,
        AudioTransportType::Virtual => 3,
        AudioTransportType::Aggregate => 4,
        AudioTransportType::Thunderbolt => 5,
        AudioTransportType::HDMI => 6,
        AudioTransportType::Unknown => 7,
    };
    Image::new_owned(MENU_ICONS[idx].clone(), MENU_ICON_SIZE, MENU_ICON_SIZE)
}

// -- Render helpers --

fn new_buf() -> Vec<u8> {
    vec![0u8; (MENU_ICON_SIZE * MENU_ICON_SIZE * 4) as usize]
}

fn set_alpha(buf: &mut [u8], x: usize, y: usize, a: f32) {
    let s = MENU_ICON_SIZE as usize;
    let i = (y * s + x) * 4;
    // Template icon: R=G=B=0, A=shape
    buf[i + 3] = (a * 255.0) as u8;
}

// -- 8 transport icon renderers (16×16) --

/// Laptop: screen rrect + base segment + feet
fn render_laptop() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Screen body (rounded rect, outline)
            let screen = sdf_rrect(px, py, 8.0, 6.5, 5.5, 4.0, 1.0).abs() - lw / 2.0;
            a = a.max(sdf_aa(screen));

            // Base line
            let base = sdf_segment(px, py, 1.5, 12.5, 14.5, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(base));

            // Left hinge
            let lh = sdf_segment(px, py, 3.0, 10.5, 2.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(lh));

            // Right hinge
            let rh = sdf_segment(px, py, 13.0, 10.5, 14.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(rh));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// USB: vertical stem + circle connector + branches with dots
fn render_usb() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Main vertical stem
            let stem = sdf_segment(px, py, 8.0, 2.0, 8.0, 13.0) - lw / 2.0;
            a = a.max(sdf_aa(stem));

            // Arrow head at top
            let arr_l = sdf_segment(px, py, 5.5, 4.5, 8.0, 2.0) - lw / 2.0;
            a = a.max(sdf_aa(arr_l));
            let arr_r = sdf_segment(px, py, 10.5, 4.5, 8.0, 2.0) - lw / 2.0;
            a = a.max(sdf_aa(arr_r));

            // Bottom circle
            let circ = sdf_circle(px, py, 8.0, 14.0, 1.5).abs() - lw / 2.0;
            a = a.max(sdf_aa(circ));

            // Right branch
            let rb = sdf_segment(px, py, 8.0, 7.0, 12.0, 9.0) - lw / 2.0;
            a = a.max(sdf_aa(rb));
            // Right branch dot (filled circle)
            let rd = sdf_circle(px, py, 12.0, 9.0, 1.2);
            a = a.max(sdf_aa(rd));

            // Left branch
            let lb = sdf_segment(px, py, 8.0, 9.5, 4.5, 11.0) - lw / 2.0;
            a = a.max(sdf_aa(lb));
            // Left branch square
            let ls = sdf_rrect(px, py, 4.5, 11.0, 1.2, 1.2, 0.2);
            a = a.max(sdf_aa(ls));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Bluetooth: the ᛒ rune shape (B-shaped path)
fn render_bluetooth() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    // Bluetooth rune scaled to 16×16:
    // Lucide bluetooth path: vertical line + two chevrons forming the B shape
    // Center at x=8, spans y=2..14
    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Vertical center line
            let vert = sdf_segment(px, py, 8.0, 2.0, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(vert));

            // Top-right chevron: center-top to right, then right to center-middle
            let tr1 = sdf_segment(px, py, 8.0, 2.0, 12.0, 5.5) - lw / 2.0;
            a = a.max(sdf_aa(tr1));
            let tr2 = sdf_segment(px, py, 12.0, 5.5, 8.0, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(tr2));

            // Bottom-right chevron: center-middle to right, then right to center-bottom
            let br1 = sdf_segment(px, py, 8.0, 8.0, 12.0, 10.5) - lw / 2.0;
            a = a.max(sdf_aa(br1));
            let br2 = sdf_segment(px, py, 12.0, 10.5, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(br2));

            // Left wings: from center-middle going out-left
            let lw1 = sdf_segment(px, py, 4.0, 5.0, 8.0, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(lw1));
            let lw2 = sdf_segment(px, py, 4.0, 11.0, 8.0, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(lw2));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Waves (Activity/Audio): 3 vertical bars of different heights (like audio-lines)
fn render_waves() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.4_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // 5 vertical bars at different heights (audio-lines style)
            let bars: [(f32, f32, f32); 5] = [
                (3.5, 5.0, 11.0),   // bar 1: x, top, bottom
                (6.0, 3.0, 13.0),   // bar 2
                (8.0, 1.5, 14.5),   // bar 3 (tallest)
                (10.0, 4.0, 12.0),  // bar 4
                (12.5, 6.0, 10.0),  // bar 5 (shortest)
            ];

            for &(bx, top, bot) in &bars {
                let seg = sdf_segment(px, py, bx, top, bx, bot) - lw / 2.0;
                a = a.max(sdf_aa(seg));
            }

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// HardDrive: box with separator line and 2 LED dots
fn render_hard_drive() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Outer box (rounded rect outline)
            let outer = sdf_rrect(px, py, 8.0, 8.0, 6.5, 5.0, 1.5).abs() - lw / 2.0;
            a = a.max(sdf_aa(outer));

            // Horizontal separator
            let sep = sdf_segment(px, py, 1.5, 8.0, 14.5, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(sep));

            // LED dots in lower half
            let led1 = sdf_circle(px, py, 10.0, 11.0, 1.0);
            a = a.max(sdf_aa(led1));
            let led2 = sdf_circle(px, py, 12.5, 11.0, 1.0);
            a = a.max(sdf_aa(led2));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Zap: lightning bolt (filled)
fn render_zap() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Lightning bolt: two triangles
            // Upper triangle: top-right to middle-left to middle-right
            let in_upper = point_in_triangle(px, py, 10.0, 1.0, 3.0, 8.5, 9.0, 8.5);
            // Lower triangle: middle-left to bottom-left to middle-right
            let in_lower = point_in_triangle(px, py, 7.0, 7.5, 6.0, 15.0, 13.0, 7.5);

            let a = if in_upper || in_lower { 1.0 } else { 0.0 };

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Monitor: screen rrect + stand + base
fn render_monitor() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Screen (rounded rect outline)
            let screen = sdf_rrect(px, py, 8.0, 5.5, 6.5, 4.0, 1.0).abs() - lw / 2.0;
            a = a.max(sdf_aa(screen));

            // Stand (vertical segment)
            let stand = sdf_segment(px, py, 8.0, 9.5, 8.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(stand));

            // Base (horizontal segment)
            let base = sdf_segment(px, py, 4.5, 12.5, 11.5, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(base));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Mic: capsule rrect + holder arc + stand
fn render_mic() -> Vec<u8> {
    let mut buf = new_buf();
    let s = MENU_ICON_SIZE as usize;
    let lw = 1.2_f32;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            // Mic capsule (filled pill shape)
            let capsule = sdf_rrect(px, py, 8.0, 5.0, 2.5, 4.0, 2.5);
            a = a.max(sdf_aa(capsule));

            // Holder arc (U-shape below capsule) — only bottom half
            if py >= 8.0 {
                let ring = sdf_circle(px, py, 8.0, 8.0, 4.5).abs() - lw / 2.0;
                a = a.max(sdf_aa(ring));
            }

            // Stand (vertical)
            let stand = sdf_segment(px, py, 8.0, 12.5, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(stand));

            // Base
            let base = sdf_segment(px, py, 5.5, 14.0, 10.5, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(base));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}
