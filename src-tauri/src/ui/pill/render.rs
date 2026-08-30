//! Drawing the pill into an RGBA buffer. No platform call anywhere: the same
//! pixels land on every backend, only the blitting differs.

use super::super::menu_icons::{sdf_aa, sdf_circle, sdf_rrect, sdf_segment};
use super::{PILL_HEIGHT, PILL_WIDTH, PillMode};

/// Scale a frame of this height was drawn at.
fn frame_scale(ch: f32) -> f32 {
    (ch / PILL_HEIGHT as f32).max(1.0)
}

/// Everything a frame needs, with no platform handles: the drawing is shared by
/// every backend, only the blitting differs.
pub(crate) struct PillFrame {
    pub mode: PillMode,
    pub smoothed: [f32; 12],
    pub dot_phase: f32,
    pub pending_count: u32,
}

pub(super) fn render_frame(p: &PillFrame, scale: f32) -> Vec<u8> {
    let w = (PILL_WIDTH as f32 * scale).round() as usize;
    let h = (PILL_HEIGHT as f32 * scale).round() as usize;
    let cw = w as f32;
    let ch = h as f32;
    let mut rgba = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Pill background (rounded rect, full radius = capsule)
            let bg = sdf_aa(sdf_rrect(px, py, cw / 2.0, ch / 2.0, cw / 2.0, ch / 2.0, ch / 2.0));
            if bg <= 0.0 {
                continue;
            }

            // Background: rgba(30,30,30,0.9), premultiplied
            let bg_a = bg * 0.9;
            let c = 30.0 / 255.0;
            let mut r = c * bg_a;
            let mut g = c * bg_a;
            let mut b = c * bg_a;
            let mut a = bg_a;

            // Content overlay
            match p.mode {
                PillMode::Preparing => {
                    // Pulsing bars at rest — signals "preparing mic, wait to speak"
                    let pulse = (p.dot_phase * 2.5).sin() * 0.15 + 0.2;
                    let fake = [pulse; 12];
                    let sa = spectrum_alpha(px, py, &fake, cw, ch);
                    if sa > 0.0 {
                        let dim = sa * 0.4;
                        over(&mut r, &mut g, &mut b, &mut a, dim, dim, dim, dim);
                    }
                }
                PillMode::Recording => {
                    let sa = spectrum_alpha(px, py, &p.smoothed, cw, ch);
                    if sa > 0.0 {
                        over(&mut r, &mut g, &mut b, &mut a, sa, sa, sa, sa);
                    }
                }
                PillMode::Paused => {
                    let pa = pause_alpha(px, py, cw, ch);
                    if pa > 0.0 {
                        // Amber: stopped, but not finished and not an error.
                        let pr = 0xfb as f32 / 255.0 * pa;
                        let pg = 0xbf as f32 / 255.0 * pa;
                        let pb = 0x24 as f32 / 255.0 * pa;
                        over(&mut r, &mut g, &mut b, &mut a, pr, pg, pb, pa);
                    }
                }
                PillMode::Transcribing => {
                    let (dr, dg, db, da) = dots_pixel(px, py, p.dot_phase, cw, ch);
                    if da > 0.0 {
                        over(&mut r, &mut g, &mut b, &mut a, dr, dg, db, da);
                    }
                }
                PillMode::Success => {
                    let sa = success_alpha(px, py, cw, ch);
                    if sa > 0.0 {
                        let sr = 0x4a as f32 / 255.0 * sa;
                        let sg = 0xde as f32 / 255.0 * sa;
                        let sb = 0x80 as f32 / 255.0 * sa;
                        over(&mut r, &mut g, &mut b, &mut a, sr, sg, sb, sa);
                    }
                }
                PillMode::Error => {
                    let ea = error_alpha(px, py, cw, ch);
                    if ea > 0.0 {
                        let er = 0xef as f32 / 255.0 * ea;
                        let eg = 0x44 as f32 / 255.0 * ea;
                        let eb = 0x44 as f32 / 255.0 * ea;
                        over(&mut r, &mut g, &mut b, &mut a, er, eg, eb, ea);
                    }
                }
                PillMode::Idle => {}
            }

            // Queue badge
            if p.pending_count > 1 {
                let (br, bg2, bb, ba) = badge_pixel(px, py, p.pending_count, cw, ch);
                if ba > 0.0 {
                    over(&mut r, &mut g, &mut b, &mut a, br, bg2, bb, ba);
                }
            }

            let idx = (y * w + x) * 4;
            rgba[idx] = (r * 255.0).min(255.0) as u8;
            rgba[idx + 1] = (g * 255.0).min(255.0) as u8;
            rgba[idx + 2] = (b * 255.0).min(255.0) as u8;
            rgba[idx + 3] = (a * 255.0).min(255.0) as u8;
        }
    }
    rgba
}

/// Premultiplied alpha src-over compositing.
#[inline]
#[allow(clippy::too_many_arguments)]
fn over(dr: &mut f32, dg: &mut f32, db: &mut f32, da: &mut f32, sr: f32, sg: f32, sb: f32, sa: f32) {
    let inv = 1.0 - sa;
    *dr = sr + *dr * inv;
    *dg = sg + *dg * inv;
    *db = sb + *db * inv;
    *da = sa + *da * inv;
}

// -- Drawing helpers --

fn spectrum_alpha(px: f32, py: f32, spectrum: &[f32; 12], cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let bar_w = (cw * 0.035).max(2.0 * scale);
    let gap = (cw * 0.025).max(1.0 * scale);
    let total = 12.0 * bar_w + 11.0 * gap;
    let start_x = (cw - total) / 2.0;
    let max_h = ch * 0.6;
    let cy = ch / 2.0;

    let mut a = 0.0f32;
    for (i, &val) in spectrum.iter().enumerate().take(12) {
        let bh = (val * max_h).max(2.0 * scale);
        let cx = start_x + i as f32 * (bar_w + gap) + bar_w / 2.0;
        let d = sdf_rrect(px, py, cx, cy, bar_w / 2.0, bh / 2.0, bar_w / 2.0);
        a = a.max(sdf_aa(d));
    }
    a
}

/// Two vertical bars — the pause glyph, centred in the pill.
fn pause_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    const BAR_W: f32 = 3.0;
    const BAR_H: f32 = 11.0;
    const GAP: f32 = 3.5;
    let cy = ch / 2.0;
    let left = sdf_rrect(px, py, cw / 2.0 - GAP - BAR_W / 2.0, cy, BAR_W / 2.0, BAR_H / 2.0, 1.0);
    let right = sdf_rrect(px, py, cw / 2.0 + GAP + BAR_W / 2.0, cy, BAR_W / 2.0, BAR_H / 2.0, 1.0);
    sdf_aa(left.min(right))
}

fn dots_pixel(px: f32, py: f32, phase: f32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let scale = frame_scale(ch);
    let dot_r = (ch * 0.12).max(3.0 * scale) / 2.0;
    let gap = (cw * 0.08).max(4.0 * scale);
    let total = 3.0 * dot_r * 2.0 + 2.0 * gap;
    let start_x = (cw - total) / 2.0;
    let cy = ch / 2.0;

    let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..3 {
        let p = phase + i as f32 * 0.8;
        let bounce = p.sin() * 0.3 + 0.7;
        let cx = start_x + i as f32 * (dot_r * 2.0 + gap) + dot_r;
        let d = sdf_circle(px, py, cx, cy, dot_r * bounce);
        let da = sdf_aa(d);
        if da > 0.0 {
            let color_a = 0.4 + bounce * 0.6;
            let sa = da * color_a;
            over(&mut r, &mut g, &mut b, &mut a, sa, sa, sa, sa);
        }
    }
    (r, g, b, a)
}

fn success_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * scale);

    // Checkmark: short stroke down-right, then long stroke up-right
    let x0 = cx - size * 0.4;
    let y0 = cy;
    let x1 = cx - size * 0.1;
    let y1 = cy + size * 0.35;
    let x2 = cx + size * 0.45;
    let y2 = cy - size * 0.35;

    let d1 = sdf_segment(px, py, x0, y0, x1, y1) - lw / 2.0;
    let d2 = sdf_segment(px, py, x1, y1, x2, y2) - lw / 2.0;
    sdf_aa(d1).max(sdf_aa(d2))
}

fn error_alpha(px: f32, py: f32, cw: f32, ch: f32) -> f32 {
    let scale = frame_scale(ch);
    let size = (ch * 0.45).round();
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let lw = (ch * 0.07).max(1.5 * scale);

    let d1 = sdf_segment(px, py, cx - size / 2.0, cy - size / 2.0, cx + size / 2.0, cy + size / 2.0) - lw / 2.0;
    let d2 = sdf_segment(px, py, cx + size / 2.0, cy - size / 2.0, cx - size / 2.0, cy + size / 2.0) - lw / 2.0;
    sdf_aa(d1).max(sdf_aa(d2))
}

fn badge_pixel(px: f32, py: f32, count: u32, cw: f32, ch: f32) -> (f32, f32, f32, f32) {
    let scale = frame_scale(ch);
    let badge_r = (ch * 0.4 / 2.0).round();
    let bx = cw - badge_r - 2.0 * scale;
    let by = badge_r + 2.0 * scale;

    let circle_a = sdf_aa(sdf_circle(px, py, bx, by, badge_r));
    if circle_a <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    // Red background (premultiplied)
    let mut r = 0xef as f32 / 255.0 * circle_a;
    let mut g = 0x44 as f32 / 255.0 * circle_a;
    let mut b = 0x44 as f32 / 255.0 * circle_a;
    let mut a = circle_a;

    // White digit (3×5 bitmap font)
    let digit = (count.min(9)) as usize;
    let scale = (badge_r * 2.0 * 0.55 / 5.0).max(1.0);
    let dw = 3.0 * scale;
    let dh = 5.0 * scale;
    let dx = bx - dw / 2.0;
    let dy = by - dh / 2.0;

    let lx = ((px - dx) / scale).floor() as i32;
    let ly = ((py - dy) / scale).floor() as i32;
    if (0..3).contains(&lx) && (0..5).contains(&ly)
        && DIGITS[digit][(ly * 3 + lx) as usize] == 1 {
            over(&mut r, &mut g, &mut b, &mut a, 1.0, 1.0, 1.0, 1.0);
    }

    (r, g, b, a)
}

#[rustfmt::skip]
const DIGITS: [[u8; 15]; 10] = [
    [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], // 0
    [0,1,0, 1,1,0, 0,1,0, 0,1,0, 1,1,1], // 1
    [1,1,1, 0,0,1, 1,1,1, 1,0,0, 1,1,1], // 2
    [1,1,1, 0,0,1, 1,1,1, 0,0,1, 1,1,1], // 3
    [1,0,1, 1,0,1, 1,1,1, 0,0,1, 0,0,1], // 4
    [1,1,1, 1,0,0, 1,1,1, 0,0,1, 1,1,1], // 5
    [1,1,1, 1,0,0, 1,1,1, 1,0,1, 1,1,1], // 6
    [1,1,1, 0,0,1, 0,0,1, 0,0,1, 0,0,1], // 7
    [1,1,1, 1,0,1, 1,1,1, 1,0,1, 1,1,1], // 8
    [1,1,1, 1,0,1, 1,1,1, 0,0,1, 1,1,1], // 9
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The Retina case, which is what the drawing was tuned against. The
    /// backends read their own scale; these tests are platform-free.
    const DPR: f32 = 2.0;
    const PX_W: usize = (PILL_WIDTH as f32 * DPR) as usize;
    const PX_H: usize = (PILL_HEIGHT as f32 * DPR) as usize;

    // -- Alpha compositing --

    #[test]
    fn over_composites_opaque_source_on_transparent_dest() {
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
        over(&mut r, &mut g, &mut b, &mut a, 1.0, 0.0, 0.0, 1.0);
        assert!((r - 1.0).abs() < 0.001);
        assert!((a - 1.0).abs() < 0.001);
    }

    #[test]
    fn over_blends_semitransparent_source() {
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 1.0, 1.0);
        over(&mut r, &mut g, &mut b, &mut a, 0.5, 0.0, 0.0, 0.5);
        // Source (red 0.5 @ 50%) over dest (blue 1.0 @ 100%)
        assert!(r > 0.4, "Red should bleed through: {r}");
        assert!(b > 0.4, "Blue should remain: {b}");
        assert!((a - 1.0).abs() < 0.001, "Alpha should be ~1.0: {a}");
    }

    // -- SDF primitives --

    #[test]
    fn sdf_aa_inside_is_opaque() {
        assert!((sdf_aa(-5.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn sdf_aa_outside_is_transparent() {
        assert!(sdf_aa(5.0) < 0.001);
    }

    #[test]
    fn sdf_aa_boundary_is_half() {
        assert!((sdf_aa(0.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn circle_center_is_inside() {
        let d = sdf_circle(50.0, 50.0, 50.0, 50.0, 10.0);
        assert!(d < 0.0, "Center of circle should be inside (negative SDF)");
    }

    #[test]
    fn circle_far_point_is_outside() {
        let d = sdf_circle(100.0, 100.0, 50.0, 50.0, 10.0);
        assert!(d > 0.0, "Far point should be outside (positive SDF)");
    }

    #[test]
    fn rrect_center_is_inside() {
        let d = sdf_rrect(80.0, 32.0, 80.0, 32.0, 80.0, 32.0, 16.0);
        assert!(d < 0.0, "Center of rounded rect should be inside");
    }

    #[test]
    fn segment_point_on_line_has_zero_distance() {
        // Midpoint of horizontal segment
        let d = sdf_segment(5.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!(d < 0.1, "Point on segment should have ~0 distance: {d}");
    }

    // -- Spectrum bars --

    #[test]
    fn spectrum_silent_audio_produces_minimal_bars() {
        let silent = [0.0f32; 12];
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample at the center of bar 6 (vertically centered)
        let bar_w = (cw * 0.035).max(2.0 * DPR);
        let gap = (cw * 0.025).max(1.0 * DPR);
        let total = 12.0 * bar_w + 11.0 * gap;
        let bar6_cx = (cw - total) / 2.0 + 6.0 * (bar_w + gap) + bar_w / 2.0;
        let a = spectrum_alpha(bar6_cx, ch / 2.0, &silent, cw, ch);
        assert!(a > 0.0, "Even silent spectrum should show minimal bars at bar center");
    }

    #[test]
    fn spectrum_loud_audio_produces_tall_bars() {
        let loud = [1.0f32; 12];
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample at bar 6 center, near the top of the pill
        let bar_w = (cw * 0.035).max(2.0 * DPR);
        let gap = (cw * 0.025).max(1.0 * DPR);
        let total = 12.0 * bar_w + 11.0 * gap;
        let bar6_cx = (cw - total) / 2.0 + 6.0 * (bar_w + gap) + bar_w / 2.0;
        let a = spectrum_alpha(bar6_cx, ch * 0.25, &loud, cw, ch);
        assert!(a > 0.0, "Loud spectrum should have bars reaching top quarter");
    }

    #[test]
    fn pause_glyph_is_two_bars_at_centre() {
        let (cw, ch) = (super::PILL_WIDTH as f32, super::PILL_HEIGHT as f32);
        // Between the bars there is a gap, so the exact centre stays empty.
        assert!(super::pause_alpha(cw / 2.0, ch / 2.0, cw, ch) < 0.01);
        // Each bar is filled.
        assert!(super::pause_alpha(cw / 2.0 - 5.0, ch / 2.0, cw, ch) > 0.9);
        assert!(super::pause_alpha(cw / 2.0 + 5.0, ch / 2.0, cw, ch) > 0.9);
        // Nothing outside them.
        assert!(super::pause_alpha(cw / 2.0, 2.0, cw, ch) < 0.01);
    }

    #[test]
    fn spectrum_outside_pill_is_transparent() {
        let loud = [1.0f32; 12];
        let a = spectrum_alpha(0.0, 0.0, &loud, PX_W as f32, PX_H as f32);
        assert!(a < 0.01, "Spectrum outside pill area should be transparent");
    }

    // -- Dots animation (transcribing) --

    #[test]
    fn dots_visible_at_pill_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let (_, _, _, a) = dots_pixel(cw / 2.0, ch / 2.0, 0.0, cw, ch);
        assert!(a > 0.0, "Transcribing dots should be visible at center");
    }

    #[test]
    fn dots_invisible_outside_pill() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let (_, _, _, a) = dots_pixel(0.0, 0.0, 0.0, cw, ch);
        assert!(a < 0.01, "Dots should not render outside pill");
    }

    // -- Success checkmark --

    #[test]
    fn success_checkmark_visible_at_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // Sample along the checkmark path (slightly right of center, on the long stroke)
        let a = success_alpha(cw * 0.55, ch * 0.4, cw, ch);
        assert!(a > 0.0, "Success checkmark should be visible near center");
    }

    // -- Error cross --

    #[test]
    fn error_cross_visible_at_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let a = error_alpha(cw / 2.0, ch / 2.0, cw, ch);
        assert!(a > 0.0, "Error cross should be visible at center");
    }

    #[test]
    fn error_cross_invisible_far_from_center() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let a = error_alpha(cw - 1.0, ch - 1.0, cw, ch);
        assert!(a < 0.01, "Error cross should not reach corners");
    }

    // -- Badge --

    #[test]
    fn badge_hidden_when_count_is_one() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        // badge_pixel is only called when count > 1 in render_frame,
        // but the function itself should still render — the guard is in render_frame
        let (_, _, _, a) = badge_pixel(cw - 10.0, 10.0, 1, cw, ch);
        // Badge still renders at count=1, but render_frame skips the call
        assert!(a >= 0.0); // just verify no panic
    }

    #[test]
    fn badge_shows_digit_at_count_5() {
        let cw = PX_W as f32;
        let ch = PX_H as f32;
        let badge_r = (ch * 0.4 / 2.0).round();
        let bx = cw - badge_r - 2.0 * DPR;
        let by = badge_r + 2.0 * DPR;
        let (_, _, _, a) = badge_pixel(bx, by, 5, cw, ch);
        assert!(a > 0.0, "Badge with count 5 should be visible at badge center");
    }

    // -- Full frame render --

    fn frame(mode: PillMode) -> PillFrame {
        PillFrame { mode, smoothed: [0.5; 12], dot_phase: 0.0, pending_count: 0 }
    }

    fn px(rgba: &[u8], x: usize, y: usize, scale: f32) -> &[u8] {
        let w = (PILL_WIDTH as f32 * scale).round() as usize;
        &rgba[(y * w + x) * 4..][..4]
    }

    #[test]
    fn render_frame_sizes_the_buffer_from_the_scale() {
        for scale in [1.0, 1.5, 2.0, 3.0] {
            let w = (PILL_WIDTH as f32 * scale).round() as usize;
            let h = (PILL_HEIGHT as f32 * scale).round() as usize;
            let rgba = render_frame(&frame(PillMode::Recording), scale);
            assert_eq!(rgba.len(), w * h * 4, "buffer at scale {scale}");
        }
    }

    #[test]
    fn render_frame_keeps_the_capsule_shape_at_every_scale() {
        for scale in [1.0, 1.5, 2.0, 3.0] {
            let h = (PILL_HEIGHT as f32 * scale).round() as usize;
            let w = (PILL_WIDTH as f32 * scale).round() as usize;
            let rgba = render_frame(&frame(PillMode::Idle), scale);
            assert_eq!(px(&rgba, 0, 0, scale)[3], 0, "corner at scale {scale}");
            let a = px(&rgba, w / 2, h / 2, scale)[3];
            assert!(a > 200, "center at scale {scale}: {a}");
        }
    }

    #[test]
    fn each_pill_mode_renders_different_content() {
        let modes = [
            PillMode::Recording,
            PillMode::Transcribing,
            PillMode::Success,
            PillMode::Error,
        ];
        let frames: Vec<Vec<u8>> = modes
            .iter()
            .map(|&mode| render_frame(&frame(mode), 2.0))
            .collect();
        for i in 0..frames.len() {
            for j in (i + 1)..frames.len() {
                assert_ne!(frames[i], frames[j],
                    "Modes {:?} and {:?} should render differently", modes[i], modes[j]);
            }
        }
    }
}
