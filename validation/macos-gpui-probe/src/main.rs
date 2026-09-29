mod input;
mod layout;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyBinding, MouseButton, MouseUpEvent, Size,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, rgb, size,
};
use gpui_platform::application;
use input::TextInputState;
use layout::{CanvasPlacement, ElementKind, ElementSpec};

const INITIAL_TITLE: &str = "日本語と English が混在するタイトル";
const SMALL_SIZE: Size<gpui::Pixels> = size(gpui::px(1024.0), gpui::px(760.0));
const LARGE_SIZE: Size<gpui::Pixels> = size(gpui::px(1440.0), gpui::px(960.0));
const TOOLBAR_HEIGHT: f32 = 144.0;
const FOOTER_HEIGHT: f32 = 72.0;
const STATE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/PROTOTYPE-title.txt");
const COORDINATE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/layout-coordinates.csv");
const IME_COORDINATE_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/ime-candidate-coordinates.csv"
);

actions!(
    presentation_probe,
    [NextSlide, PreviousSlide, ExitPresentation]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayMode {
    Editing,
    Presentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorSize {
    Small,
    Large,
}

struct ValidationApp {
    title: gpui::Entity<TextInputState>,
    root_focus: FocusHandle,
    display_mode: DisplayMode,
    editor_size: EditorSize,
    current_slide: usize,
    title_editing: bool,
    overflow_fixed: bool,
    saved_title: String,
    last_action: String,
    save_successes: usize,
    save_blocked: usize,
    presentation_blocked: usize,
    fullscreen_transitions: usize,
    last_coordinate_key: Option<String>,
}

impl ValidationApp {
    fn new(cx: &mut Context<Self>, title: gpui::Entity<TextInputState>) -> Self {
        cx.observe(&title, |_, _, cx| cx.notify()).detach();
        Self {
            title,
            root_focus: cx.focus_handle(),
            display_mode: DisplayMode::Editing,
            editor_size: EditorSize::Small,
            current_slide: 0,
            title_editing: true,
            overflow_fixed: false,
            saved_title: INITIAL_TITLE.to_owned(),
            last_action: "タイトル欄で標準日本語 IME を操作してください。".to_owned(),
            save_successes: 0,
            save_blocked: 0,
            presentation_blocked: 0,
            fullscreen_transitions: 0,
            last_coordinate_key: None,
        }
    }

    fn title_value(&self, cx: &Context<Self>) -> String {
        self.title.read(cx).value()
    }

    fn title_is_valid(&self, cx: &Context<Self>) -> bool {
        let value = self.title_value(cx);
        !value.trim().is_empty() && !value.contains('\0')
    }

    fn current_elements(&self, cx: &Context<Self>) -> Vec<ElementSpec> {
        let heading = self.title_value(cx);
        layout::slide_elements(self.current_slide, &heading, self.overflow_fixed)
    }

    fn has_current_overflow(&self, cx: &Context<Self>) -> bool {
        layout::has_overflow(&self.current_elements(cx))
    }

    fn set_small_size(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.resize_editor(EditorSize::Small, window, cx);
    }

    fn set_large_size(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.resize_editor(EditorSize::Large, window, cx);
    }

    fn resize_editor(&mut self, size: EditorSize, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_maximized() {
            window.zoom_window();
        }
        window.resize(match size {
            EditorSize::Small => SMALL_SIZE,
            EditorSize::Large => LARGE_SIZE,
        });
        self.editor_size = size;
        self.last_coordinate_key = None;
        self.last_action = match size {
            EditorSize::Small => "編集表示を小さいサイズへ変更しました。".to_owned(),
            EditorSize::Large => "編集表示を大きいサイズへ変更しました。".to_owned(),
        };
        cx.notify();
    }

    fn select_slide_0(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.select_slide(0, cx);
    }

    fn select_slide_1(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.select_slide(1, cx);
    }

    fn select_slide_2(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.select_slide(2, cx);
    }

    fn select_slide(&mut self, slide: usize, cx: &mut Context<Self>) {
        self.current_slide = slide;
        self.last_coordinate_key = None;
        self.last_action = format!("Slide {} を表示しました。", slide + 1);
        cx.notify();
    }

    fn begin_title_edit(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.title_editing = true;
        let focus = self.title.read(cx).focus_handle();
        window.focus(&focus, cx);
        self.last_action = "タイトル欄へフォーカスを戻しました。".to_owned();
        cx.notify();
    }

    fn make_title_invalid(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.title.update(cx, |title, cx| title.set_value("", cx));
        self.title_editing = true;
        let focus = self.title.read(cx).focus_handle();
        window.focus(&focus, cx);
        self.last_action = "空のタイトルを設定しました。保存と発表は停止します。".to_owned();
        cx.notify();
    }

    fn fix_title(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.title
            .update(cx, |title, cx| title.set_value(INITIAL_TITLE, cx));
        self.title_editing = true;
        let focus = self.title.read(cx).focus_handle();
        window.focus(&focus, cx);
        self.last_action = "有効なタイトルへ修正しました。".to_owned();
        cx.notify();
    }

    fn cancel_title_edit(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.saved_title.clone();
        self.title
            .update(cx, |title, cx| title.set_value(&value, cx));
        self.title_editing = false;
        let focus = self.root_focus.clone();
        window.focus(&focus, cx);
        self.last_action = "入力を保存済みの値へ戻して取消しました。".to_owned();
        cx.notify();
    }

    fn fix_overflow(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.overflow_fixed = true;
        self.last_coordinate_key = None;
        self.last_action = "Slide の内容を短くし、はみ出し診断を解除しました。".to_owned();
        cx.notify();
    }

    fn save(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.root_focus.clone();
        window.focus(&focus, cx);
        let value = self.title_value(cx);
        let marked = self.title.read(cx).is_marked();
        if marked || !self.title_is_valid(cx) {
            self.save_blocked += 1;
            self.last_action = if marked {
                "IME の未確定文字列が残るため保存を止めました。".to_owned()
            } else {
                "タイトルが無効なため保存を止めました。修正または取消してください。".to_owned()
            };
            cx.notify();
            return;
        }
        match fs::write(STATE_FILE, value.as_bytes()) {
            Ok(()) => {
                self.saved_title = value;
                self.save_successes += 1;
                self.title_editing = false;
                self.last_action = "確定した文字列を PROTOTYPE ファイルへ保存しました。".to_owned();
            }
            Err(error) => {
                self.last_action = format!("PROTOTYPE ファイルへの保存に失敗しました: {error}");
            }
        }
        cx.notify();
    }

    fn start_from_first(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.start_presentation(0, window, cx);
    }

    fn start_from_current(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_presentation(self.current_slide, window, cx);
    }

    fn start_presentation(&mut self, slide: usize, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.root_focus.clone();
        window.focus(&focus, cx);
        let marked = self.title.read(cx).is_marked();
        if marked || !self.title_is_valid(cx) {
            self.presentation_blocked += 1;
            self.last_action = if marked {
                "IME の変換を確定できなかったため発表を止めました。".to_owned()
            } else {
                "タイトルが無効なため発表を止めました。修正または取消してください。".to_owned()
            };
            cx.notify();
            return;
        }
        self.current_slide = slide.min(2);
        if self.has_current_overflow(cx) {
            self.presentation_blocked += 1;
            self.last_action = "はみ出し診断が残るため発表を止めました。".to_owned();
            cx.notify();
            return;
        }
        self.title_editing = false;
        self.display_mode = DisplayMode::Presentation;
        self.fullscreen_transitions += 1;
        self.last_coordinate_key = None;
        self.last_action = if slide == 0 {
            "最初の Slide から全画面発表を開始しました。".to_owned()
        } else {
            format!("現在の Slide {} から全画面発表を開始しました。", slide + 1)
        };
        window.toggle_fullscreen();
        let focus = self.root_focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    fn next_slide(&mut self, _: &NextSlide, _: &mut Window, cx: &mut Context<Self>) {
        if self.display_mode != DisplayMode::Presentation {
            return;
        }
        self.current_slide = (self.current_slide + 1).min(2);
        self.last_coordinate_key = None;
        self.last_action = format!("次の Slide: {} / 3", self.current_slide + 1);
        cx.notify();
    }

    fn previous_slide(&mut self, _: &PreviousSlide, _: &mut Window, cx: &mut Context<Self>) {
        if self.display_mode != DisplayMode::Presentation {
            return;
        }
        self.current_slide = self.current_slide.saturating_sub(1);
        self.last_coordinate_key = None;
        self.last_action = format!("前の Slide: {} / 3", self.current_slide + 1);
        cx.notify();
    }

    fn exit_presentation(
        &mut self,
        _: &ExitPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.display_mode != DisplayMode::Presentation {
            return;
        }
        window.toggle_fullscreen();
        self.display_mode = DisplayMode::Editing;
        self.title_editing = false;
        self.fullscreen_transitions += 1;
        self.last_coordinate_key = None;
        self.last_action = format!("Slide {} の編集表示へ戻りました。", self.current_slide + 1);
        let focus = self.root_focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    fn render_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let slide = self.current_slide + 1;
        let current_overflow = self.has_current_overflow(cx);
        let title_valid = self.title_is_valid(cx);
        let dirty = self.title_value(cx) != self.saved_title;
        let toolbar = div()
            .flex()
            .flex_col()
            .gap_2()
            .h(gpui::px(TOOLBAR_HEIGHT))
            .px_3()
            .py_2()
            .bg(rgb(0xf1f2f4))
            .child(
                div()
                    .text_size(gpui::px(13.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("PROTOTYPE / 問い: 標準日本語 IME・共通配置・全画面復帰を GPUI で満たせるか。"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(control_button(
                        cx,
                        "小さい編集表示",
                        true,
                        Self::set_small_size,
                    ))
                    .child(control_button(
                        cx,
                        "大きい編集表示",
                        true,
                        Self::set_large_size,
                    ))
                    .child(control_button(cx, "1 / 1:1", true, Self::select_slide_0))
                    .child(control_button(cx, "2 / 1:2", true, Self::select_slide_1))
                    .child(control_button(
                        cx,
                        "3 / はみ出し",
                        true,
                        Self::select_slide_2,
                    ))
                    .child(control_button(
                        cx,
                        "タイトルを編集",
                        true,
                        Self::begin_title_edit,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(control_button(cx, "保存", title_valid, Self::save))
                    .child(control_button(
                        cx,
                        "最初から発表",
                        title_valid && !current_overflow,
                        Self::start_from_first,
                    ))
                    .child(control_button(
                        cx,
                        "現在から発表",
                        title_valid && !current_overflow,
                        Self::start_from_current,
                    ))
                    .child(control_button(
                        cx,
                        "無効入力",
                        true,
                        Self::make_title_invalid,
                    ))
                    .child(control_button(cx, "タイトル修正", true, Self::fix_title))
                    .child(control_button(
                        cx,
                        "入力取消",
                        true,
                        Self::cancel_title_edit,
                    ))
                    .child(control_button(
                        cx,
                        "はみ出し修正",
                        current_overflow,
                        Self::fix_overflow,
                    )),
            );
        let viewport = window.viewport_size();
        let canvas_height = (f32::from(viewport.height) - TOOLBAR_HEIGHT - FOOTER_HEIGHT).max(1.0);
        let canvas = self.render_canvas(
            window,
            cx,
            DisplayMode::Editing,
            size(viewport.width, gpui::px(canvas_height)),
            TOOLBAR_HEIGHT,
        );
        let (undo_count, redo_count) = self.title.read(cx).history_counts();
        let (enter_count, escape_count, arrow_count) = self.title.read(cx).action_counts();
        let marked = self.title.read(cx).is_marked();
        let focused = self.title.read(cx).has_focus(window);
        let window_bounds = window.bounds();
        let viewport = window.viewport_size();
        let status = div()
            .flex()
            .flex_col()
            .h(gpui::px(FOOTER_HEIGHT))
            .px_3()
            .py_1()
            .bg(rgb(0xf7f7f8))
            .text_size(gpui::px(13.0))
            .child(format!(
                "表示: 編集 / Slide {slide}/3 / {} / 保存状態: {} / IME 未確定: {} / タイトル欄フォーカス: {}",
                match self.editor_size {
                    EditorSize::Small => "小さいサイズ",
                    EditorSize::Large => "大きいサイズ",
                },
                if dirty { "未保存" } else { "保存済み" },
                if marked { "あり" } else { "なし" },
                if focused { "あり" } else { "なし" },
            ))
            .child(format!(
                "Undo/Redo: {undo_count}/{redo_count} / 変換中の Enter/Escape/矢印 action: {enter_count}/{escape_count}/{arrow_count} / 保存成功・停止: {}/{} / 発表停止: {} / 全画面遷移: {} / {}",
                self.save_successes,
                self.save_blocked,
                self.presentation_blocked,
                self.fullscreen_transitions,
                self.last_action,
            ))
            .child(format!(
                "GPUI window: {} × {} px / viewport: {} × {} px / maximized: {}",
                f32::from(window_bounds.size.width),
                f32::from(window_bounds.size.height),
                f32::from(viewport.width),
                f32::from(viewport.height),
                if window.is_maximized() { "あり" } else { "なし" },
            ));
        div()
            .flex()
            .flex_col()
            .size_full()
            .key_context("Editor")
            .track_focus(&self.root_focus)
            .bg(rgb(0xe8eaed))
            .child(toolbar)
            .child(div().relative().flex_1().w_full().child(canvas))
            .child(status)
    }

    fn render_presentation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current_overflow = self.has_current_overflow(cx);
        let canvas = self.render_canvas(
            window,
            cx,
            DisplayMode::Presentation,
            window.viewport_size(),
            0.0,
        );
        div()
            .relative()
            .size_full()
            .key_context("Presentation")
            .track_focus(&self.root_focus)
            .on_action(cx.listener(Self::next_slide))
            .on_action(cx.listener(Self::previous_slide))
            .on_action(cx.listener(Self::exit_presentation))
            .bg(rgb(0xffffff))
            .child(canvas)
            .when(current_overflow, |element| {
                element.child(
                    div()
                        .absolute()
                        .top(gpui::px(12.0))
                        .right(gpui::px(12.0))
                        .px_2()
                        .py_1()
                        .bg(rgb(0xffdddd))
                        .text_color(rgb(0x9c1d1d))
                        .child("はみ出し診断あり"),
                )
            })
    }

    fn render_canvas(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        mode: DisplayMode,
        available: Size<gpui::Pixels>,
        canvas_top: f32,
    ) -> gpui::AnyElement {
        let placement = CanvasPlacement::fit(available);
        let elements = self.current_elements(cx);
        self.record_coordinates(window, mode, available, placement, canvas_top, &elements);
        let scale = placement.scale;
        let editing_title =
            self.title_editing && mode == DisplayMode::Editing && self.current_slide == 0;
        let title_entity = self.title.clone();
        let rendered_elements = elements.into_iter().map(move |element| {
            let bounds = placement.bounds(&element);
            let item = div()
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .text_size(gpui::px(element.font_size * scale))
                .line_height(gpui::px(element.line_height * scale))
                .text_color(rgb(0x222222));
            match element.kind {
                ElementKind::Heading => {
                    let item = item
                        .font_family("Hiragino Sans")
                        .font_weight(gpui::FontWeight::BOLD);
                    if editing_title {
                        item.child(title_entity.clone())
                    } else {
                        item.child(element.text)
                    }
                }
                ElementKind::Code => item
                    .font_family("Menlo")
                    .bg(rgb(0xf1f2f3))
                    .p(gpui::px(14.0 * scale))
                    .child(element.text),
                ElementKind::Caption => item
                    .font_family("Hiragino Sans")
                    .text_color(rgb(0x4c5157))
                    .child(element.text),
                ElementKind::Image => item
                    .font_family("Hiragino Sans")
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgb(0xe5e8eb))
                    .border_1()
                    .border_color(rgb(0xb8bec5))
                    .child(element.text),
                ElementKind::Text | ElementKind::Bullets => {
                    item.font_family("Hiragino Sans").child(element.text)
                }
            }
        });
        div()
            .relative()
            .size_full()
            .bg(rgb(0xffffff))
            .children(rendered_elements)
            .into_any_element()
    }

    fn record_coordinates(
        &mut self,
        window: &Window,
        mode: DisplayMode,
        available: Size<gpui::Pixels>,
        placement: CanvasPlacement,
        canvas_top: f32,
        elements: &[ElementSpec],
    ) {
        let viewport = window.viewport_size();
        let window_bounds = window.bounds();
        let mode_name = match mode {
            DisplayMode::Editing => match self.editor_size {
                EditorSize::Small => "editing-small",
                EditorSize::Large => "editing-large",
            },
            DisplayMode::Presentation => "fullscreen",
        };
        let key = format!(
            "{mode_name}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.current_slide,
            f32::from(viewport.width),
            f32::from(viewport.height),
            f32::from(available.width),
            f32::from(available.height),
            f32::from(window_bounds.origin.x),
            f32::from(window_bounds.origin.y),
            f32::from(window_bounds.size.width),
            f32::from(window_bounds.size.height),
            self.overflow_fixed,
        );
        if self.last_coordinate_key.as_deref() == Some(&key) {
            return;
        }
        self.last_coordinate_key = Some(key);
        let is_new = !Path::new(COORDINATE_FILE).exists();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(COORDINATE_FILE)
        {
            if is_new {
                let _ = writeln!(
                    file,
                    "mode,slide,viewport_width,viewport_height,window_x,window_y,window_width,window_height,element,kind,base_x,base_y,base_width,base_height,canvas_scale,canvas_x,canvas_y,window_scale,pixel_x,pixel_y,pixel_width,pixel_height"
                );
            }
            let display_scale = window.scale_factor();
            for element in elements {
                let bounds = placement.bounds(element);
                let x = f32::from(bounds.origin.x);
                let y = canvas_top + f32::from(bounds.origin.y);
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                let window_x = f32::from(window_bounds.origin.x);
                let window_y = f32::from(window_bounds.origin.y);
                let window_width = f32::from(window_bounds.size.width);
                let window_height = f32::from(window_bounds.size.height);
                let viewport_width = f32::from(viewport.width);
                let viewport_height = f32::from(viewport.height);
                let kind = format!("{:?}", element.kind).to_lowercase();
                let _ = writeln!(
                    file,
                    "{mode_name},{},{viewport_width},{viewport_height},{window_x},{window_y},{window_width},{window_height},{},{kind},{},{},{},{},{},{x},{y},{display_scale},{},{},{},{}",
                    self.current_slide + 1,
                    element.id,
                    element.x,
                    element.y,
                    element.width,
                    element.height,
                    placement.scale,
                    x * display_scale,
                    y * display_scale,
                    width * display_scale,
                    height * display_scale,
                );
            }
        }
    }
}

impl Render for ValidationApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.display_mode {
            DisplayMode::Editing => self.render_editor(window, cx).into_any_element(),
            DisplayMode::Presentation => self.render_presentation(window, cx).into_any_element(),
        }
    }
}

impl Focusable for ValidationApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.root_focus.clone()
    }
}

fn control_button(
    cx: &mut Context<ValidationApp>,
    label: &'static str,
    enabled: bool,
    handler: impl Fn(&mut ValidationApp, &MouseUpEvent, &mut Window, &mut Context<ValidationApp>)
    + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(gpui::px(13.0))
        .bg(if enabled {
            rgb(0xffffff)
        } else {
            rgb(0xe5e5e5)
        })
        .border_1()
        .border_color(if enabled {
            rgb(0xb3b8bf)
        } else {
            rgb(0xd0d0d0)
        })
        .cursor_pointer()
        .child(label)
        .on_mouse_up(MouseButton::Left, cx.listener(handler))
}

fn main() {
    let parent = Path::new(STATE_FILE).parent().unwrap();
    fs::create_dir_all(parent).expect("create prototype output directory");
    fs::write(STATE_FILE, INITIAL_TITLE).expect("reset prototype title file");
    fs::write(
        COORDINATE_FILE,
        "mode,slide,viewport_width,viewport_height,window_x,window_y,window_width,window_height,element,kind,base_x,base_y,base_width,base_height,canvas_scale,canvas_x,canvas_y,window_scale,pixel_x,pixel_y,pixel_width,pixel_height\n",
    )
    .expect("reset layout coordinate log");
    fs::write(
        IME_COORDINATE_FILE,
        "range_start_utf16,range_end_utf16,left,top,right,bottom,pixel_left,pixel_top,window_scale\n",
    )
    .expect("reset IME candidate coordinate log");

    application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("right", NextSlide, Some("Presentation")),
            KeyBinding::new("down", NextSlide, Some("Presentation")),
            KeyBinding::new("space", NextSlide, Some("Presentation")),
            KeyBinding::new("pagedown", NextSlide, Some("Presentation")),
            KeyBinding::new("left", PreviousSlide, Some("Presentation")),
            KeyBinding::new("up", PreviousSlide, Some("Presentation")),
            KeyBinding::new("pageup", PreviousSlide, Some("Presentation")),
            KeyBinding::new("escape", ExitPresentation, Some("Presentation")),
            KeyBinding::new("backspace", input::Backspace, Some("TextInput")),
            KeyBinding::new("delete", input::Delete, Some("TextInput")),
            KeyBinding::new("left", input::Left, Some("TextInput")),
            KeyBinding::new("right", input::Right, Some("TextInput")),
            KeyBinding::new("up", input::Up, Some("TextInput")),
            KeyBinding::new("down", input::Down, Some("TextInput")),
            KeyBinding::new("cmd-a", input::SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-z", input::Undo, Some("TextInput")),
            KeyBinding::new("cmd-shift-z", input::Redo, Some("TextInput")),
            KeyBinding::new("enter", input::Enter, Some("TextInput")),
            KeyBinding::new("escape", input::Escape, Some("TextInput")),
        ]);

        let bounds = Bounds::centered(None, SMALL_SIZE, cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let title = cx.new(|cx| TextInputState::new(cx, INITIAL_TITLE));
                    cx.new(|cx| ValidationApp::new(cx, title))
                },
            )
            .expect("open GPUI validation window");
        window
            .update(cx, |view, window, cx| {
                let focus = view.title.read(cx).focus_handle();
                window.focus(&focus, cx);
                cx.activate(true);
            })
            .expect("focus title field");
    });
}
