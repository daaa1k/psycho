use gpui::{Hsla, Pixels, Size, WindowAppearance, px, rgb, size};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomMode {
    Fit,
    Actual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
    Navigator,
    Inspector,
}
#[derive(Clone, Copy)]
pub struct PaneView {
    pub visible: bool,
    pub preferred_width: f32,
}
#[derive(Clone, Copy)]
pub struct PaneResize {
    pub pane: Pane,
    pub start_x: f32,
    pub start_width: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Popup {
    Slide(gpui::Point<Pixels>),
    Insert,
    View,
}

pub struct EditorViewState {
    pub navigator: PaneView,
    pub inspector: PaneView,
    pub zoom: ZoomMode,
    pub canvas_area: Size<Pixels>,
    pub resize: Option<PaneResize>,
    pub popup: Option<Popup>,
}
impl Default for EditorViewState {
    fn default() -> Self {
        Self {
            navigator: PaneView {
                visible: true,
                preferred_width: 160.,
            },
            inspector: PaneView {
                visible: true,
                preferred_width: 280.,
            },
            zoom: ZoomMode::Fit,
            canvas_area: size(px(1.), px(1.)),
            resize: None,
            popup: None,
        }
    }
}
impl EditorViewState {
    pub fn pane(&mut self, pane: Pane) -> &mut PaneView {
        match pane {
            Pane::Navigator => &mut self.navigator,
            Pane::Inspector => &mut self.inspector,
        }
    }
    pub fn widths(&self, available: f32) -> (f32, f32) {
        let navigator = if self.navigator.visible {
            self.navigator.preferred_width.clamp(120., 360.)
        } else {
            0.
        };
        let inspector = if self.inspector.visible {
            self.inspector.preferred_width.clamp(220., 440.)
        } else {
            0.
        };
        let panes = navigator + inspector;
        let ratio = if panes > 0. {
            ((available - 320.).max(0.) / panes).min(1.)
        } else {
            1.
        };
        (navigator * ratio, inspector * ratio)
    }
    pub fn scale(&self) -> f32 {
        match self.zoom {
            ZoomMode::Actual => 1.,
            ZoomMode::Fit => (f32::from(self.canvas_area.width) / 1280.)
                .min(f32::from(self.canvas_area.height) / 720.)
                .max(0.001),
        }
    }
}

#[derive(Clone, Copy)]
pub struct EditorPalette {
    pub background: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub secondary: Hsla,
    pub border: Hsla,
    pub selection: Hsla,
    pub focus: Hsla,
    pub warning: Hsla,
    pub input: Hsla,
}
impl gpui::Global for EditorPalette {}
impl EditorPalette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        if matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ) {
            Self {
                background: rgb(0x181b20).into(),
                surface: rgb(0x252a32).into(),
                text: rgb(0xf2f4f8).into(),
                secondary: rgb(0xbac3d1).into(),
                border: rgb(0x68758a).into(),
                selection: rgb(0x304565).into(),
                focus: rgb(0x91c4ff).into(),
                warning: rgb(0xffb9ab).into(),
                input: rgb(0x11151b).into(),
            }
        } else {
            Self {
                background: rgb(0xe9edf2).into(),
                surface: rgb(0xf6f8fb).into(),
                text: rgb(0x1c2533).into(),
                secondary: rgb(0x4d596b).into(),
                border: rgb(0x8994a4).into(),
                selection: rgb(0xdce8fb).into(),
                focus: rgb(0x174f9b).into(),
                warning: rgb(0x922b1c).into(),
                input: rgb(0xffffff).into(),
            }
        }
    }
}

pub struct IconAssets;
impl gpui::AssetSource for IconAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let icon: Option<&'static [u8]> = match path {
            "icons/add-slide.svg" => Some(include_bytes!("../assets/icons/add-slide.svg")),
            "icons/insert.svg" => Some(include_bytes!("../assets/icons/insert.svg")),
            "icons/view.svg" => Some(include_bytes!("../assets/icons/view.svg")),
            "icons/present.svg" => Some(include_bytes!("../assets/icons/present.svg")),
            "icons/undo.svg" => Some(include_bytes!("../assets/icons/undo.svg")),
            "icons/redo.svg" => Some(include_bytes!("../assets/icons/redo.svg")),
            _ => None,
        };
        Ok(icon.map(std::borrow::Cow::Borrowed))
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shrinking_window_keeps_preferred_pane_widths() {
        let mut view = EditorViewState::default();
        view.navigator.preferred_width = 360.;
        view.inspector.preferred_width = 440.;
        let (left, right) = view.widths(1024.);
        assert!(left + right <= 704.);
        assert_eq!(view.widths(1440.), (360., 440.));
        view.navigator.visible = false;
        assert_eq!(view.widths(1024.), (0., 440.));
    }
    #[test]
    fn fit_uses_both_actual_dimensions_and_actual_size_is_unscaled() {
        let mut view = EditorViewState::default();
        view.canvas_area = size(px(640.), px(720.));
        assert_eq!(view.scale(), 0.5);
        view.canvas_area = size(px(1280.), px(180.));
        assert_eq!(view.scale(), 0.25);
        view.zoom = ZoomMode::Actual;
        assert_eq!(view.scale(), 1.);
    }
}
