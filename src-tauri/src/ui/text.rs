//! Text rasteriser shared by every overlay.
//!
//! The subtitle strip draws itself into a buffer rather than handing text to a
//! native control, so the same glyphs, the same wrapping and the same
//! antialiasing land on macOS and Windows.

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle, WrapStyle};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

const INTER: &[u8] = include_bytes!("../../fonts/Inter-Regular.ttf");

/// Families to try first when Inter cannot draw a character. Seven engines
/// transcribe Chinese, Japanese and Korean, which Inter does not cover; picking
/// the first covering face off the system would land on whatever the scan
/// happened to reach, so name the faces each OS actually ships for CJK.
#[cfg(target_os = "macos")]
const FALLBACK_FAMILIES: &[&str] =
    &["PingFang SC", "Hiragino Sans", "Apple SD Gothic Neo", "Geeza Pro", "Thonburi"];
#[cfg(not(target_os = "macos"))]
const FALLBACK_FAMILIES: &[&str] =
    &["Microsoft YaHei", "Yu Gothic", "Malgun Gothic", "Segoe UI", "Nirmala UI"];

/// A rasterised block of text: coverage per pixel, for the caller to composite
/// in whatever colour it draws with.
pub(crate) struct TextImage {
    pub alpha: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub lines: usize,
}

struct FontSet {
    fonts: Vec<Font>,
    db: Option<fontdb::Database>,
    /// Resolved once per character, misses included — the system scan behind a
    /// miss is far too expensive to repeat for every frame of a live preview.
    resolved: HashMap<char, usize>,
}

static FONTS: LazyLock<Mutex<FontSet>> = LazyLock::new(|| Mutex::new(FontSet::new()));

impl FontSet {
    fn new() -> Self {
        let inter = Font::from_bytes(INTER, FontSettings::default())
            .expect("the bundled Inter is valid");
        Self { fonts: vec![inter], db: None, resolved: HashMap::new() }
    }

    fn index_for(&mut self, c: char) -> usize {
        if let Some(&i) = self.resolved.get(&c) {
            return i;
        }
        let index = if self.fonts[0].lookup_glyph_index(c) != 0 {
            0
        } else {
            self.load_fallback(c).unwrap_or(0)
        };
        self.resolved.insert(c, index);
        index
    }

    fn load_fallback(&mut self, c: char) -> Option<usize> {
        let db = self.db.get_or_insert_with(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        });

        let mut candidates: Vec<(usize, fontdb::ID, u32)> = Vec::new();
        for face in db.faces() {
            let rank = FALLBACK_FAMILIES
                .iter()
                .position(|want| face.families.iter().any(|(name, _)| name == want));
            if let Some(rank) = rank {
                candidates.push((rank, face.id, face.index));
            }
        }
        candidates.sort_by_key(|(rank, _, _)| *rank);

        for (_, id, index) in candidates {
            let data = db.with_face_data(id, |data, _| data.to_vec())?;
            let settings = FontSettings { collection_index: index, ..Default::default() };
            let Ok(font) = Font::from_bytes(data, settings) else { continue };
            if font.lookup_glyph_index(c) == 0 {
                continue;
            }
            self.fonts.push(font);
            return Some(self.fonts.len() - 1);
        }
        log::debug!("Subtitle: no font covers U+{:04X}", c as u32);
        None
    }
}

/// Lay out and rasterise `text`, wrapping at `width`. The image is exactly
/// `width` wide and as tall as the wrapped text turned out to need.
pub(crate) fn render(text: &str, px: f32, width: usize, line_height: f32) -> TextImage {
    let mut set = FONTS.lock().unwrap();
    let mut layout: Layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: Some(width as f32),
        wrap_style: WrapStyle::Word,
        line_height: line_height / px,
        ..Default::default()
    });

    // One run per stretch of characters sharing a font: fontdue picks the glyph
    // from the style's font_index, so a mixed-script line needs the text split.
    let mut run = String::new();
    let mut run_font = 0usize;
    for c in text.chars() {
        let font = set.index_for(c);
        if font != run_font && !run.is_empty() {
            layout.append(&set.fonts, &TextStyle::new(&run, px, run_font));
            run.clear();
        }
        run_font = font;
        run.push(c);
    }
    if !run.is_empty() {
        layout.append(&set.fonts, &TextStyle::new(&run, px, run_font));
    }

    let lines = layout.lines().map_or(0, |l| l.len());
    // Descenders on the last line reach below its line box — a cedilla or a "p"
    // loses its tail if the canvas stops at lines * line_height.
    let deepest = layout
        .glyphs()
        .iter()
        .map(|g| g.y + g.height as f32)
        .fold(0.0f32, f32::max);
    let height = (lines as f32 * line_height).max(deepest).ceil().max(line_height) as usize;
    let mut alpha = vec![0.0f32; width * height];

    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        let (_, coverage) = set.fonts[glyph.font_index].rasterize_config(glyph.key);
        for row in 0..glyph.height {
            let y = glyph.y as isize + row as isize;
            if y < 0 || y as usize >= height {
                continue;
            }
            for col in 0..glyph.width {
                let x = glyph.x as isize + col as isize;
                if x < 0 || x as usize >= width {
                    continue;
                }
                let value = coverage[row * glyph.width + col] as f32 / 255.0;
                let slot = &mut alpha[y as usize * width + x as usize];
                *slot = slot.max(value);
            }
        }
    }

    TextImage { alpha, width, height, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_latin_text_into_the_requested_width() {
        let img = render("Bonjour", 15.0, 200, 19.0);
        assert_eq!(img.width, 200);
        assert_eq!(img.lines, 1);
        assert_eq!(img.alpha.len(), img.width * img.height);
        assert!(img.alpha.iter().any(|&a| a > 0.5), "des pixels sont couverts");
    }

    #[test]
    fn wraps_onto_several_lines_when_the_width_runs_out() {
        let narrow = render("Bonjour ceci est une phrase de test", 15.0, 90, 19.0);
        let wide = render("Bonjour ceci est une phrase de test", 15.0, 600, 19.0);
        assert!(narrow.lines > wide.lines);
        assert!(narrow.height > wide.height);
    }

    #[test]
    fn the_canvas_makes_room_for_a_descender_on_the_last_line() {
        // Retina scale, where the strip actually draws: 15pt text, 19pt lines.
        let wrapped = "Bonjour ceci est un apercu de la bande de sous-titres avec un \u{e7}";
        let img = render(wrapped, 30.0, 500, 38.0);
        assert!(img.lines > 1, "le texte doit passer a la ligne");
        assert!(
            img.height > (img.lines as f32 * 38.0) as usize,
            "la cedille de la derniere ligne deborde de sa boite: {} px pour {} lignes",
            img.height,
            img.lines
        );
    }

    #[test]
    fn empty_text_still_yields_one_line_of_canvas() {
        let img = render("", 15.0, 200, 19.0);
        assert_eq!(img.height, 19);
        assert!(img.alpha.iter().all(|&a| a == 0.0));
    }

    #[test]
    fn accented_french_stays_on_the_bundled_font() {
        let mut set = FONTS.lock().unwrap();
        for c in "éèêàçùôïœ".chars() {
            assert_eq!(set.index_for(c), 0, "{c} devrait venir d'Inter");
        }
    }
}
