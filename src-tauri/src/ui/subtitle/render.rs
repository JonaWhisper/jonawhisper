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

    let cap_lines = cap.max(1) as usize;
    let img = text::render(content, FONT_SIZE as f32 * scale, text_width, line_px, cap_lines);

    let points = height_for_lines(img.lines as f64, cap);
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
            let ty = y as f32 - pad;
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

    /// Truncated transcripts used to start at pixel 0: the strip dropped the
    /// oldest lines by shifting the canvas a multiple of the line height, while
    /// fontdue had placed them on its own metrics.
    #[test]
    fn a_truncated_transcript_keeps_its_top_margin() {
        let long = "Alors, ce que je voudrais faire, c'est afficher le texte pendant que je \
                    parle, avec beaucoup de mots pour depasser la limite de lignes.";
        for cap in [1u8, 2, 3] {
            let strip = render_strip(long, 2.0, cap);
            let inked = |y: usize| {
                (0..strip.width).any(|x| strip.rgba[(y * strip.width + x) * 4] > 120)
            };
            let first = (0..strip.height).find(|&y| inked(y)).expect("de l'encre");
            assert!(
                first >= (PADDING as usize),
                "cap {cap}: le texte commence a {first}, sous la marge de {PADDING}"
            );
        }
    }

    #[test]
    fn the_text_keeps_its_margins_top_and_bottom() {
        let strip = render_strip("Bonjour, ceci deborde jusqu'a la derniere ligne visible.", 2.0, 5);
        let row_has_ink = |y: usize| {
            (0..strip.width).any(|x| strip.rgba[(y * strip.width + x) * 4] > 120)
        };
        let pad = (PADDING * 2.0) as usize; // en pixels, scale 2
        let first = (0..strip.height).find(|&y| row_has_ink(y)).expect("de l'encre");
        let last = (0..strip.height).rev().find(|&y| row_has_ink(y)).expect("de l'encre");
        assert!(first > 0, "une marge subsiste en haut");
        assert!(
            strip.height - 1 - last > 0,
            "une marge subsiste en bas: encre jusqu'a {last} sur {}",
            strip.height
        );
        // et les deux marges restent comparables — c'est l'asymetrie qui se voyait
        let top = first;
        let bottom = strip.height - 1 - last;
        assert!(
            top.abs_diff(bottom) <= pad / 2,
            "marges deséquilibrées: {top} en haut, {bottom} en bas"
        );
    }

    #[test]
    fn text_is_lighter_than_the_backdrop() {
        let strip = render_strip("IIIIIIIIIIIIIIII", 2.0, 5);
        let brightest = strip.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).max().unwrap();
        assert!(brightest > 100, "de l'encre blanche est visible: {brightest}");
    }
}


