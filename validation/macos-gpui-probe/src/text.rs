//! Shape and wrap at the shared 1280x720 size. Scale glyph origins only at paint.
use gpui::{
    App, FontId, GlyphId, Hsla, Pixels, Point, TextRun, Window, canvas, point, prelude::*, px,
};

use crate::layout::{ElementKind, ElementSpec};

pub fn expand_tabs(text: &str) -> String {
    let mut result = String::new();
    let mut column = 0;
    for ch in text.chars() {
        match ch {
            '\n' => {
                result.push(ch);
                column = 0;
            }
            '\t' => {
                let spaces = 4 - column % 4;
                result.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            }
            _ => {
                result.push(ch);
                column += 1;
            }
        }
    }
    result
}

struct Glyph {
    font: FontId,
    id: GlyphId,
    origin: Point<Pixels>,
    font_size: Pixels,
    color: Hsla,
}

pub fn shared_text(element: ElementSpec, scale: f32, origin: Point<Pixels>) -> impl IntoElement {
    canvas(
        move |_, window, _| {
            let mut evidence = String::new();
            let mut visible_glyph_index = 0;
            let text = if element.kind == ElementKind::Code {
                expand_tabs(&element.text)
            } else {
                element.text.clone()
            };
            let family = if element.kind == ElementKind::Code {
                "Menlo"
            } else {
                "Hiragino Sans"
            };
            let mut font = gpui::font(family);
            if element.kind == ElementKind::Heading {
                font.weight = gpui::FontWeight::BOLD;
            }
            let color: Hsla = gpui::rgb(if element.kind == ElementKind::Caption {
                0x4c5157
            } else {
                0x222222
            })
            .into();
            let run = TextRun {
                len: text.len(),
                font,
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let wrap_width = (element.kind != ElementKind::Code).then(|| px(element.width));
            let lines = window
                .text_system()
                .shape_text(text.into(), px(element.font_size), &[run], wrap_width, None)
                .expect("shape shared text");
            let mut glyphs = Vec::new();
            let mut visual_line = 0;
            for line in lines {
                let layout = &line.unwrapped_layout;
                let baseline =
                    (px(element.line_height) - layout.ascent - layout.descent) / 2. + layout.ascent;
                let image_x = if element.kind == ElementKind::Image {
                    (px(element.width) - layout.width) / 2.
                } else {
                    px(0.)
                };
                let image_y = if element.kind == ElementKind::Image {
                    (element.height - element.line_height) / 2.
                } else {
                    0.
                };
                let mut wrap_start = px(0.);
                for (run_ix, run) in layout.runs.iter().enumerate() {
                    for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                        if line.wrap_boundaries.iter().any(|boundary| {
                            boundary.run_ix == run_ix && boundary.glyph_ix == glyph_ix
                        }) {
                            wrap_start = glyph.position.x;
                            visual_line += 1;
                        }
                        let padding = if element.kind == ElementKind::Code {
                            16.0
                        } else {
                            0.0
                        };
                        let origin = origin
                            + point(px(padding * scale), px(padding * scale))
                            + point(
                                (image_x + glyph.position.x - wrap_start) * scale,
                                (px(image_y + visual_line as f32 * element.line_height)
                                    + baseline
                                    + glyph.position.y)
                                    * scale,
                            );
                        if !line.text[glyph.index..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                        {
                            use std::fmt::Write;
                            let viewport = window.viewport_size();
                            let _ = writeln!(
                                evidence,
                                "{},{},{},{},{},{},{},{},{}",
                                f32::from(viewport.width),
                                f32::from(viewport.height),
                                element.id,
                                visible_glyph_index,
                                glyph.id.0,
                                f32::from(origin.x),
                                f32::from(origin.y),
                                element
                                    .text
                                    .as_bytes()
                                    .iter()
                                    .map(|byte| format!("{byte:02x}"))
                                    .collect::<String>(),
                                window.scale_factor()
                            );
                            visible_glyph_index += 1;
                        }
                        glyphs.push(Glyph {
                            font: run.font_id,
                            id: glyph.id,
                            origin,
                            font_size: px(element.font_size * scale),
                            color,
                        });
                    }
                }
                visual_line += 1;
            }
            if let Some(path) = std::env::var_os("PSYCHO_GLYPH_TRACE") {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("open glyph evidence");
                file.write_all(evidence.as_bytes())
                    .expect("write glyph evidence");
            }
            glyphs
        },
        |_, glyphs, window: &mut Window, _: &mut App| {
            for glyph in glyphs {
                window
                    .paint_glyph(
                        glyph.origin,
                        glyph.font,
                        glyph.id,
                        glyph.font_size,
                        glyph.color,
                    )
                    .expect("paint shared glyph");
            }
        },
    )
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::expand_tabs;
    #[test]
    fn tabs_advance_to_four_column_stops_and_reset_after_newline() {
        assert_eq!(expand_tabs("\t日本語\nx\ty\t"), "    日本語\nx   y   ");
    }
}
