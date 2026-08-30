//! Composing the strip: the rounded backdrop, and the text rasterised into it.
//! No platform call — the same pixels land on every backend.

use super::super::menu_icons::{sdf_aa, sdf_rrect};
use super::super::text;
use super::{BACKDROP_ALPHA, CORNER_RADIUS, FONT_SIZE, GREY, LINE_HEIGHT, PADDING, WIDTH};


/// Height for a given number of lines, padding included.
pub(super) fn height_for_lines(lines: f64, cap: u8) -> f64 {
    lines.clamp(1.0, cap.max(1) as f64) * LINE_HEIGHT + PADDING * 2.0
}

pub(super) struct Strip {
    pub(super) rgba: Vec<u8>,
    pub(super) width: usize,
    pub(super) height: usize,
    /// What the buffer measures once drawn at `scale`. AppKit sizes its window
    /// in points; the Windows overlay works in pixels from end to end.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(super) points: f64,
}

/// Compose the rounded backdrop and the text into one premultiplied buffer.
pub(super) fn render_strip(content: &str, scale: f32, cap: u8) -> Strip {
    let pad = (PADDING as f32 * scale).round();
    let line_px = LINE_HEIGHT as f32 * scale;
    let width = (WIDTH as f32 * scale).round() as usize;
    let text_width = (width as f32 - pad * 2.0).max(1.0) as usize;

    let img = text::render(content, FONT_SIZE as f32 * scale, text_width, line_px);
    let visible = img.lines.clamp(1, cap.max(1) as usize);
    // Show the tail, not the head: on a live transcript the newest words are
    // the ones the user is checking, and they are at the bottom.
    let skipped = img.lines.saturating_sub(visible) as f32 * line_px;

    let points = height_for_lines(visible as f64, cap);
    let height = (points as f32 * scale).round() as usize;
    let mut rgba = vec![0u8; width * height * 4];
    let (cw, ch) = (width as f32, height as f32);
    let radius = CORNER_RADIUS as f32 * scale;

    for y in 0..height {
        for x in 0..width {
            let backdrop = sdf_aa(sdf_rrect(
                x as f32 + 0.5,
                y as f32 + 0.5,
                cw / 2.0,
                ch / 2.0,
                cw / 2.0,
                ch / 2.0,
                radius,
            ));
            if backdrop <= 0.0 {
                continue;
            }
            let back_a = backdrop * BACKDROP_ALPHA;

            let tx = x as f32 - pad;
            let ty = y as f32 - pad + skipped;
            let ink = if tx >= 0.0 && ty >= 0.0 {
                let (tx, ty) = (tx as usize, ty as usize);
                if tx < img.width && ty < img.height {
                    img.alpha[ty * img.width + tx] * backdrop
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // White text over the backdrop, both premultiplied.
            let a = back_a * (1.0 - ink) + ink;
            let c = GREY * back_a * (1.0 - ink) + ink;
            let i = (y * width + x) * 4;
            rgba[i] = (c * 255.0).round() as u8;
            rgba[i + 1] = (c * 255.0).round() as u8;
            rgba[i + 2] = (c * 255.0).round() as u8;
            rgba[i + 3] = (a * 255.0).round() as u8;
        }
    }

    Strip { rgba, width, height, points }
}


#[cfg(test)]
mod tests {
    use super::super::TOP_OFFSET;
    use super::*;

    #[test]
    fn height_honours_the_configured_cap() {
        let one = height_for_lines(1.0, 5);
        assert_eq!(height_for_lines(0.0, 5), one, "jamais moins d'une ligne");
        assert_eq!(height_for_lines(99.0, 5), height_for_lines(5.0, 5), "plafonne au reglage");
        assert_eq!(height_for_lines(99.0, 2), height_for_lines(2.0, 2));
        assert!(height_for_lines(99.0, 2) < height_for_lines(5.0, 5));
        assert_eq!(height_for_lines(3.0, 0), one, "0 ne fait pas disparaitre la bande");
    }

    #[test]
    fn sits_below_the_pill() {
        // The pill occupies 40..72 from the top; the strip must clear it.
        const { assert!(TOP_OFFSET >= 40.0 + 32.0) };
    }

    #[test]
    fn strip_is_transparent_at_the_corners_and_opaque_inside() {
        let strip = render_strip("Bonjour", 2.0, 5);
        assert_eq!(strip.rgba.len(), strip.width * strip.height * 4);
        assert_eq!(strip.rgba[3], 0, "coin arrondi transparent");
        let centre = (strip.height / 2 * strip.width + strip.width / 2) * 4;
        assert!(strip.rgba[centre + 3] > 200, "fond opaque au centre");
    }

    #[test]
    fn strip_grows_with_the_text_then_stops_at_the_cap() {
        let short = render_strip("Bonjour", 2.0, 2);
        let long = render_strip(&"mot ".repeat(200), 2.0, 2);
        assert!(long.points > short.points, "la bande grandit");
        assert_eq!(long.points, height_for_lines(2.0, 2), "puis plafonne");
    }

    #[test]
    fn text_is_lighter_than_the_backdrop() {
        let strip = render_strip("IIIIIIIIIIIIIIII", 2.0, 5);
        let brightest = strip.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).max().unwrap();
        assert!(brightest > 100, "de l'encre blanche est visible: {brightest}");
    }
}

