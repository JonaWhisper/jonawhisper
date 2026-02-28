use crate::platform::audio_devices::AudioTransportType;
use std::sync::LazyLock;
use tauri::image::Image;

// Shape cache: 16×16 alpha-only (used as source for compositing)
const SHAPE_SIZE: usize = 16;
// Output icon size: 36×36 pixels = 18pt @2x Retina (muda uses fixed_height=18)
const MENU_ICON_SIZE: u32 = 36;

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

// -- Cached icon shapes (16×16 alpha buffers) --

static ICON_SHAPES: LazyLock<[Vec<u8>; 8]> = LazyLock::new(|| {
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

/// Bilinear sample from a 16×16 alpha buffer (RGBA, only A channel used).
fn sample_shape(shape: &[u8], fx: f32, fy: f32) -> f32 {
    if fx < 0.0 || fy < 0.0 || fx >= SHAPE_SIZE as f32 || fy >= SHAPE_SIZE as f32 {
        return 0.0;
    }
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(SHAPE_SIZE - 1);
    let y1 = (y0 + 1).min(SHAPE_SIZE - 1);
    let dx = fx - fx.floor();
    let dy = fy - fy.floor();

    let a00 = shape[(y0 * SHAPE_SIZE + x0) * 4 + 3] as f32 / 255.0;
    let a10 = shape[(y0 * SHAPE_SIZE + x1) * 4 + 3] as f32 / 255.0;
    let a01 = shape[(y1 * SHAPE_SIZE + x0) * 4 + 3] as f32 / 255.0;
    let a11 = shape[(y1 * SHAPE_SIZE + x1) * 4 + 3] as f32 / 255.0;

    let top = a00 * (1.0 - dx) + a10 * dx;
    let bot = a01 * (1.0 - dx) + a11 * dx;
    top * (1.0 - dy) + bot * dy
}

// Bubble colors
const BLUE: (u8, u8, u8) = (0, 122, 255);   // macOS system blue
const GRAY: (u8, u8, u8) = (174, 174, 178);  // macOS systemGray3

/// Get a 36×36 icon with colored bubble for an audio transport type.
/// `selected`: blue bubble (active device), otherwise gray bubble.
pub fn transport_icon(t: &AudioTransportType, selected: bool) -> Image<'static> {
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
    let shape = &ICON_SHAPES[idx];
    let (bg_r, bg_g, bg_b) = if selected { BLUE } else { GRAY };

    let s = MENU_ICON_SIZE as usize;
    let mut rgba = vec![0u8; s * s * 4];

    // Bubble: filled rounded rect (squircle feel) covering most of the 36×36 area
    // Icon shape: sampled from 16×16 cache, mapped to a 22×22 area centered in the bubble
    let center = s as f32 / 2.0;
    let bubble_r = center - 1.5; // radius 16.5 in a 36×36 canvas
    let icon_margin = 7.0; // icon occupies center 22×22 area (36 - 2*7 = 22)
    let icon_span = s as f32 - 2.0 * icon_margin;

    for y in 0..s {
        for x in 0..s {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Bubble shape (filled circle)
            let bubble_a = sdf_aa(sdf_circle(px, py, center, center, bubble_r));
            if bubble_a <= 0.0 {
                continue;
            }

            // Sample the icon shape (maps icon_margin..icon_margin+icon_span → 0..16)
            let ix = (px - icon_margin) * SHAPE_SIZE as f32 / icon_span;
            let iy = (py - icon_margin) * SHAPE_SIZE as f32 / icon_span;
            let icon_a = sample_shape(shape, ix, iy);

            // Composite: white icon on colored bubble
            let r = bg_r as f32 * (1.0 - icon_a) + 255.0 * icon_a;
            let g = bg_g as f32 * (1.0 - icon_a) + 255.0 * icon_a;
            let b = bg_b as f32 * (1.0 - icon_a) + 255.0 * icon_a;

            let i = (y * s + x) * 4;
            rgba[i] = r as u8;
            rgba[i + 1] = g as u8;
            rgba[i + 2] = b as u8;
            rgba[i + 3] = (bubble_a * 255.0) as u8;
        }
    }

    Image::new_owned(rgba, MENU_ICON_SIZE, MENU_ICON_SIZE)
}

// -- Shape renderers (16×16 alpha-only, cached) --

fn new_shape_buf() -> Vec<u8> {
    vec![0u8; SHAPE_SIZE * SHAPE_SIZE * 4]
}

fn set_alpha(buf: &mut [u8], x: usize, y: usize, a: f32) {
    let i = (y * SHAPE_SIZE + x) * 4;
    buf[i + 3] = (a * 255.0) as u8;
}

/// Laptop: screen rrect + base segment + hinges
fn render_laptop() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let screen = sdf_rrect(px, py, 8.0, 6.5, 5.5, 4.0, 1.0).abs() - lw / 2.0;
            a = a.max(sdf_aa(screen));
            let base = sdf_segment(px, py, 1.5, 12.5, 14.5, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(base));
            let lh = sdf_segment(px, py, 3.0, 10.5, 2.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(lh));
            let rh = sdf_segment(px, py, 13.0, 10.5, 14.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(rh));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// USB: vertical stem + arrow + branches with dots
fn render_usb() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let stem = sdf_segment(px, py, 8.0, 2.0, 8.0, 13.0) - lw / 2.0;
            a = a.max(sdf_aa(stem));
            let arr_l = sdf_segment(px, py, 5.5, 4.5, 8.0, 2.0) - lw / 2.0;
            a = a.max(sdf_aa(arr_l));
            let arr_r = sdf_segment(px, py, 10.5, 4.5, 8.0, 2.0) - lw / 2.0;
            a = a.max(sdf_aa(arr_r));
            let circ = sdf_circle(px, py, 8.0, 14.0, 1.5).abs() - lw / 2.0;
            a = a.max(sdf_aa(circ));
            let rb = sdf_segment(px, py, 8.0, 7.0, 12.0, 9.0) - lw / 2.0;
            a = a.max(sdf_aa(rb));
            let rd = sdf_circle(px, py, 12.0, 9.0, 1.2);
            a = a.max(sdf_aa(rd));
            let lb = sdf_segment(px, py, 8.0, 9.5, 4.5, 11.0) - lw / 2.0;
            a = a.max(sdf_aa(lb));
            let ls = sdf_rrect(px, py, 4.5, 11.0, 1.2, 1.2, 0.2);
            a = a.max(sdf_aa(ls));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Bluetooth: the rune B shape
fn render_bluetooth() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let vert = sdf_segment(px, py, 8.0, 2.0, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(vert));
            let tr1 = sdf_segment(px, py, 8.0, 2.0, 12.0, 5.5) - lw / 2.0;
            a = a.max(sdf_aa(tr1));
            let tr2 = sdf_segment(px, py, 12.0, 5.5, 8.0, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(tr2));
            let br1 = sdf_segment(px, py, 8.0, 8.0, 12.0, 10.5) - lw / 2.0;
            a = a.max(sdf_aa(br1));
            let br2 = sdf_segment(px, py, 12.0, 10.5, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(br2));
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

/// Waves: 5 vertical bars at different heights
fn render_waves() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.4_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let bars: [(f32, f32, f32); 5] = [
                (3.5, 5.0, 11.0),
                (6.0, 3.0, 13.0),
                (8.0, 1.5, 14.5),
                (10.0, 4.0, 12.0),
                (12.5, 6.0, 10.0),
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

/// HardDrive: box + separator + LEDs
fn render_hard_drive() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let outer = sdf_rrect(px, py, 8.0, 8.0, 6.5, 5.0, 1.5).abs() - lw / 2.0;
            a = a.max(sdf_aa(outer));
            let sep = sdf_segment(px, py, 1.5, 8.0, 14.5, 8.0) - lw / 2.0;
            a = a.max(sdf_aa(sep));
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
    let mut buf = new_shape_buf();

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let in_upper = point_in_triangle(px, py, 10.0, 1.0, 3.0, 8.5, 9.0, 8.5);
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
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let screen = sdf_rrect(px, py, 8.0, 5.5, 6.5, 4.0, 1.0).abs() - lw / 2.0;
            a = a.max(sdf_aa(screen));
            let stand = sdf_segment(px, py, 8.0, 9.5, 8.0, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(stand));
            let base = sdf_segment(px, py, 4.5, 12.5, 11.5, 12.5) - lw / 2.0;
            a = a.max(sdf_aa(base));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}

/// Mic: capsule + holder arc + stand
fn render_mic() -> Vec<u8> {
    let mut buf = new_shape_buf();
    let lw = 1.2_f32;

    for y in 0..SHAPE_SIZE {
        for x in 0..SHAPE_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut a = 0.0_f32;

            let capsule = sdf_rrect(px, py, 8.0, 5.0, 2.5, 4.0, 2.5);
            a = a.max(sdf_aa(capsule));
            if py >= 8.0 {
                let ring = sdf_circle(px, py, 8.0, 8.0, 4.5).abs() - lw / 2.0;
                a = a.max(sdf_aa(ring));
            }
            let stand = sdf_segment(px, py, 8.0, 12.5, 8.0, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(stand));
            let base = sdf_segment(px, py, 5.5, 14.0, 10.5, 14.0) - lw / 2.0;
            a = a.max(sdf_aa(base));

            if a > 0.0 {
                set_alpha(&mut buf, x, y, a);
            }
        }
    }
    buf
}
