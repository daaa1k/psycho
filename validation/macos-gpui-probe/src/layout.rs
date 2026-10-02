use gpui::{Bounds, Pixels, Size, point, px, size};

pub const SLIDE_WIDTH: f32 = 1280.0;
pub const SLIDE_HEIGHT: f32 = 720.0;
pub const MARGIN_X: f32 = 64.0;
pub const MARGIN_Y: f32 = 48.0;
pub const ELEMENT_GAP: f32 = 24.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementKind {
    Heading,
    Text,
    Bullets,
    Code,
    Image,
    Caption,
}

#[derive(Clone, Debug)]
pub struct ElementSpec {
    pub id: &'static str,
    pub kind: ElementKind,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub line_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasPlacement {
    pub scale: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl CanvasPlacement {
    pub fn fit(available: Size<Pixels>) -> Self {
        let width = f32::from(available.width).max(1.0);
        let height = f32::from(available.height).max(1.0);
        let scale = (width / SLIDE_WIDTH).min(height / SLIDE_HEIGHT);
        Self {
            scale,
            origin_x: (width - SLIDE_WIDTH * scale) / 2.0,
            origin_y: (height - SLIDE_HEIGHT * scale) / 2.0,
        }
    }

    pub fn bounds(self, element: &ElementSpec) -> Bounds<Pixels> {
        Bounds::new(
            point(
                px(self.origin_x + element.x * self.scale),
                px(self.origin_y + element.y * self.scale),
            ),
            size(
                px(element.width * self.scale),
                px(element.height * self.scale),
            ),
        )
    }
}

fn element(
    id: &'static str,
    kind: ElementKind,
    text: impl Into<String>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    font_size: f32,
    line_height: f32,
) -> ElementSpec {
    ElementSpec {
        id,
        kind,
        text: text.into(),
        x,
        y,
        width,
        height,
        font_size,
        line_height,
    }
}

pub fn slide_elements(
    slide: usize,
    editable_heading: &str,
    overflow_fixed: bool,
) -> Vec<ElementSpec> {
    match slide {
        0 => {
            let left_width = (SLIDE_WIDTH - 2.0 * MARGIN_X - ELEMENT_GAP) / 2.0;
            let right_x = MARGIN_X + left_width + ELEMENT_GAP;
            vec![
                element(
                    "heading",
                    ElementKind::Heading,
                    editable_heading,
                    MARGIN_X,
                    MARGIN_Y,
                    SLIDE_WIDTH - 2.0 * MARGIN_X,
                    60.0,
                    48.0,
                    60.0,
                ),
                element(
                    "mixed-text-and-url",
                    ElementKind::Text,
                    "日本語と English が混在する本文です。\nhttps://example.org/psycho/presentation/layout/long-url-wrap-check-2026/reconversion-candidate-position-and-text-editing-coordinate-validation/scale-boundary-and-two-column-wrap-observation",
                    MARGIN_X,
                    134.0,
                    SLIDE_WIDTH - 2.0 * MARGIN_X,
                    136.0,
                    26.0,
                    38.4,
                ),
                element(
                    "bullets-1-to-1",
                    ElementKind::Bullets,
                    "• 箇条書きの行頭と折返し\n• 左右の列幅を比較する\n• Caption は画像の下に置く",
                    MARGIN_X,
                    280.0,
                    left_width,
                    122.0,
                    28.0,
                    42.0,
                ),
                element(
                    "tabbed-code",
                    ElementKind::Code,
                    "fn main() {\n\tprintln!(\"日本語 IME を確認\");\n}",
                    MARGIN_X,
                    418.0,
                    left_width,
                    172.0,
                    22.0,
                    30.8,
                ),
                element(
                    "image-1-to-1",
                    ElementKind::Image,
                    "画像枠 1:1",
                    right_x,
                    280.0,
                    left_width,
                    300.0,
                    20.0,
                    28.0,
                ),
                element(
                    "caption-1-to-1",
                    ElementKind::Caption,
                    "Caption: 編集と発表で同じ画像説明を表示する。",
                    right_x,
                    592.0,
                    left_width,
                    50.0,
                    20.0,
                    28.0,
                ),
            ]
        }
        1 => {
            let usable_width = SLIDE_WIDTH - 2.0 * MARGIN_X - ELEMENT_GAP;
            let left_width = usable_width / 3.0;
            let right_x = MARGIN_X + left_width + ELEMENT_GAP;
            let right_width = usable_width - left_width;
            vec![
                element(
                    "heading",
                    ElementKind::Heading,
                    "比率 1:2 の列配置",
                    MARGIN_X,
                    MARGIN_Y,
                    SLIDE_WIDTH - 2.0 * MARGIN_X,
                    60.0,
                    48.0,
                    60.0,
                ),
                element(
                    "body-1-to-2",
                    ElementKind::Text,
                    "Column widths remain fixed at 1:2 while the slide scales.",
                    MARGIN_X,
                    134.0,
                    SLIDE_WIDTH - 2.0 * MARGIN_X,
                    56.0,
                    28.0,
                    42.0,
                ),
                element(
                    "bullets-1-to-2",
                    ElementKind::Bullets,
                    "• 左列\n• 幅 1\n• 日本語",
                    MARGIN_X,
                    264.0,
                    left_width,
                    150.0,
                    28.0,
                    42.0,
                ),
                element(
                    "code-1-to-2",
                    ElementKind::Code,
                    "let columns = (1, 2);\nlet layout = shared_for_editor_and_presenter(columns);",
                    right_x,
                    264.0,
                    right_width,
                    138.0,
                    22.0,
                    30.8,
                ),
                element(
                    "image-1-to-2",
                    ElementKind::Image,
                    "画像枠 1:2",
                    right_x,
                    430.0,
                    right_width,
                    160.0,
                    20.0,
                    28.0,
                ),
                element(
                    "caption-1-to-2",
                    ElementKind::Caption,
                    "Caption: 右列は左列の2倍幅。",
                    right_x,
                    602.0,
                    right_width,
                    50.0,
                    20.0,
                    28.0,
                ),
            ]
        }
        _ if overflow_fixed => vec![
            element(
                "heading",
                ElementKind::Heading,
                "はみ出しを修正した Slide",
                MARGIN_X,
                MARGIN_Y,
                SLIDE_WIDTH - 2.0 * MARGIN_X,
                60.0,
                48.0,
                60.0,
            ),
            element(
                "overflow-text",
                ElementKind::Text,
                "内容を短くして表示領域に収めました。\n保存と発表を再開できます。",
                MARGIN_X,
                144.0,
                SLIDE_WIDTH - 2.0 * MARGIN_X,
                120.0,
                28.0,
                42.0,
            ),
        ],
        _ => vec![
            element(
                "heading",
                ElementKind::Heading,
                "はみ出し診断の検証",
                MARGIN_X,
                MARGIN_Y,
                SLIDE_WIDTH - 2.0 * MARGIN_X,
                60.0,
                48.0,
                60.0,
            ),
            element(
                "overflow-text",
                ElementKind::Text,
                "この文章の下端は表示領域を越えます。\n発表開始を止め、診断対象を示します。\n修正後は診断を解除します。\n表示サイズを変えても基準位置は固定します。\n日本語と English が混在しています。\n行を追加してはみ出しを再現します。\n診断は保存を妨げず、発表開始だけを止めます。\n最終行は Slide の下端を越えることを確認します。\n追加行 09\n追加行 10\n追加行 11\n追加行 12\n追加行 13\n追加行 14\n追加行 15",
                MARGIN_X,
                144.0,
                SLIDE_WIDTH - 2.0 * MARGIN_X,
                690.0,
                28.0,
                42.0,
            ),
        ],
    }
}

pub fn has_overflow(elements: &[ElementSpec]) -> bool {
    elements.iter().any(|element| {
        element.x < MARGIN_X
            || element.y < MARGIN_Y
            || element.x + element.width > SLIDE_WIDTH - MARGIN_X
            || element.y + element.height > SLIDE_HEIGHT - MARGIN_Y
    })
}
