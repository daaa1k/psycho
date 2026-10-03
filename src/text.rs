//! Shape at the presentation's base size, then scale the same glyph positions.
use gpui::{App, Hsla, Pixels, Point, TextRun, Window, canvas, point, prelude::*, px};
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub struct TextPlacement {
    pub canvas_origin: Rc<Cell<Point<Pixels>>>,
    offset: Point<Pixels>,
}

impl TextPlacement {
    pub fn new(canvas_origin: Rc<Cell<Point<Pixels>>>, x: f32, y: f32) -> Self {
        Self {
            canvas_origin,
            offset: point(px(x), px(y)),
        }
    }

    pub fn origin(&self, scale: f32) -> Point<Pixels> {
        self.canvas_origin.get() + self.offset * scale
    }

    pub fn shifted(&self, x: f32, y: f32) -> Self {
        Self {
            canvas_origin: self.canvas_origin.clone(),
            offset: self.offset + point(px(x), px(y)),
        }
    }
}

pub fn expand_tabs(text: &str) -> String {
    let mut expanded = String::new();
    let mut column = 0;
    for ch in text.chars() {
        match ch {
            '\n' => {
                expanded.push(ch);
                column = 0;
            }
            '\t' => {
                let spaces = 4 - column % 4;
                expanded.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            }
            _ => {
                expanded.push(ch);
                column += 1;
            }
        }
    }
    expanded
}

pub fn shared_text(
    text: &str,
    family: &str,
    font_size: f32,
    line_height: f32,
    width: f32,
    bold: bool,
    highlights: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)>,
    scale: f32,
    window: &Window,
    placement: &TextPlacement,
) -> gpui::AnyElement {
    let mut font = gpui::font(family);
    if bold {
        font.weight = gpui::FontWeight::BOLD;
    }
    let color: Hsla = gpui::rgb(0x222222).into();
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let lines = window
        .text_system()
        .shape_text(
            text.to_owned().into(),
            px(font_size),
            &[run],
            width.is_finite().then(|| px(width)),
            None,
        )
        .expect("shape presentation text");
    let height = lines
        .iter()
        .map(|line| f32::from(line.size(px(line_height)).height))
        .sum::<f32>()
        .max(line_height);
    let text = text.to_owned();
    let placement = placement.clone();
    canvas(
        move |_, _, _| {
            let mut glyphs = Vec::new();
            let mut visual_line = 0;
            let mut byte_offset = 0;
            for line in &lines {
                let layout = &line.unwrapped_layout;
                let baseline =
                    (px(line_height) - layout.ascent - layout.descent) / 2. + layout.ascent;
                let mut wrap_start = px(0.);
                for (run_ix, run) in layout.runs.iter().enumerate() {
                    for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                        if line.wrap_boundaries.iter().any(|boundary| {
                            boundary.run_ix == run_ix && boundary.glyph_ix == glyph_ix
                        }) {
                            wrap_start = glyph.position.x;
                            visual_line += 1;
                        }
                        let color = highlights
                            .iter()
                            .find(|(range, _)| range.contains(&(byte_offset + glyph.index)))
                            .and_then(|(_, style)| style.color)
                            .unwrap_or(color);
                        let origin = point(
                            (glyph.position.x - wrap_start) * scale,
                            (px(visual_line as f32 * line_height) + baseline + glyph.position.y)
                                * scale,
                        );
                        glyphs.push((run.font_id, glyph.id, origin, color));
                    }
                }
                visual_line += 1;
                byte_offset += line.text.len() + 1;
            }
            glyphs
        },
        move |_, glyphs, window: &mut Window, _: &mut App| {
            let mut evidence = String::new();
            let tracing = std::env::var_os("PSYCHO_GLYPH_TRACE");
            let text_hex = tracing.as_ref().map(|_| {
                text.as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            });
            let base_origin = placement.canvas_origin.get();
            for (index, (font, glyph, relative, color)) in glyphs.into_iter().enumerate() {
                let origin = base_origin + placement.offset * scale + relative;
                if let Some(text_hex) = &text_hex {
                    use std::fmt::Write;
                    let viewport = window.viewport_size();
                    let _ = writeln!(
                        evidence,
                        "{},{},{},{},{},{},{},{},{},{},{},{}",
                        f32::from(viewport.width),
                        f32::from(viewport.height),
                        font_size,
                        text_hex,
                        index,
                        glyph.0,
                        f32::from(origin.x),
                        f32::from(origin.y),
                        scale,
                        f32::from(base_origin.x),
                        f32::from(base_origin.y),
                        window.scale_factor()
                    );
                }
                window
                    .paint_glyph(origin, font, glyph, px(font_size * scale), color)
                    .expect("paint presentation glyph");
            }
            if let Some(path) = tracing {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = file.write_all(evidence.as_bytes());
                }
            }
        },
    )
    .w_full()
    .h(px(height * scale))
    .flex_shrink_0()
    .into_any_element()
}
