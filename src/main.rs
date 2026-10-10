#![allow(unexpected_cfgs)]

mod ime;
mod input;
mod text;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, DragMoveEvent, FocusHandle, Focusable, InteractiveElement, KeyBinding,
    Menu, MenuItem, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use input::TextInputState;
use objc::{msg_send, runtime::Object, sel, sel_impl};
use psycho::{
    AssetDiagnostic, Diagnostic, DiagnosticKind, DocumentError, Element, ElementField, ElementKind,
    ExternalState, PresentationDocument, PresentationModel,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

actions!(
    psycho,
    [
        SaveFile,
        SaveAs,
        UndoEdit,
        RedoEdit,
        NextSlide,
        PreviousSlide,
        ExitPresentation,
        DeleteSelection,
        CommitTextEdit
    ]
);

const DEMO: &str = include_str!("../examples/build-time.kdl");
const WINDOW_SIZE: gpui::Size<gpui::Pixels> = size(px(1280.0), px(860.0));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditTarget {
    Title,
    Element {
        slide: usize,
        index: usize,
        field: ElementField,
    },
    Columns {
        slide: usize,
        index: usize,
    },
    ColumnWidth {
        slide: usize,
        index: usize,
        left: bool,
    },
    NestedElement {
        slide: usize,
        columns: usize,
        column: usize,
        index: usize,
        field: ElementField,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasTarget {
    Element {
        slide: usize,
        index: usize,
    },
    NestedElement {
        slide: usize,
        columns: usize,
        column: usize,
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasSource {
    Element {
        slide: usize,
        index: usize,
    },
    NestedElement {
        slide: usize,
        columns: usize,
        column: usize,
        index: usize,
    },
}

#[derive(Clone, Copy, Debug)]
struct DraggedCanvasElement {
    source: CanvasSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DropDestination {
    Element {
        slide: usize,
        index: usize,
        after: bool,
    },
    NestedElement {
        slide: usize,
        columns: usize,
        column: usize,
        index: usize,
        after: bool,
    },
    Column {
        slide: usize,
        columns: usize,
        column: usize,
    },
}

struct DragGhost;

impl gpui::Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .child("Element")
    }
}

#[derive(Clone, Copy, Default)]
struct OutlinePage {
    slide: usize,
    index: usize,
}

struct PsychoApp {
    document: PresentationDocument,
    editor: gpui::Entity<TextInputState>,
    root_focus: FocusHandle,
    ime_geometry: ime::Geometry,
    current_slide: usize,
    outline_page: OutlinePage,
    target: EditTarget,
    canvas_editing: bool,
    column_add_menu: Option<usize>,
    presentation: Option<PresentationModel>,
    presentation_slide: usize,
    return_slide: usize,
    presentation_cursor_hidden: bool,
    cursor_hide_generation: u64,
    status: String,
    pending_open: Option<PathBuf>,
    pending_close: bool,
    pending_external_reload: bool,
    save_as_pending: bool,
    source_selection: Option<Diagnostic>,
    layout_diagnostics: Vec<LayoutDiagnostic>,
    asset_diagnostics: Vec<AssetDiagnostic>,
    asset_images: HashMap<String, Arc<gpui::RenderImage>>,
    presentation_assets: Option<HashMap<String, Arc<gpui::RenderImage>>>,
    column_resize_drag: Option<ColumnResizeDrag>,
    drop_destination: Option<DropDestination>,
}

#[derive(Clone, Copy)]
struct ColumnResizeDrag {
    slide: usize,
    index: usize,
    start_x: f32,
    start_width: u8,
    preview_width: u8,
    usable_width: f32,
}

#[derive(Clone)]
struct LayoutDiagnostic {
    slide_index: usize,
    element_index: usize,
    message: String,
}

impl PsychoApp {
    fn new(
        cx: &mut Context<Self>,
        document: PresentationDocument,
        editor: gpui::Entity<TextInputState>,
        window: &mut Window,
    ) -> Self {
        let ime_rect = ime::Geometry::default();
        let input_ime_rect = ime_rect.clone();
        cx.observe_in(&editor, window, move |_, editor, window, cx| {
            if editor.read(cx).has_focus(window) {
                input_ime_rect.refresh(window, false);
            }
            cx.notify();
        })
        .detach();
        cx.observe_window_bounds(window, move |this, window, cx| {
            if this.editor.read(cx).has_focus(window) {
                this.ime_geometry.refresh(window, true);
            }
        })
        .detach();
        let input_focus = editor.read(cx).focus_handle();
        cx.on_blur(&input_focus, window, |this, window, cx| {
            if !this.save_as_pending
                && this.presentation.is_none()
                && !this.external_edit_blocked()
                && (this.has_uncommitted_draft(cx)
                    || this.editor.read(cx).can_undo()
                    || this.editor.read(cx).can_redo())
            {
                if let Err(error) = this.commit_draft(window, cx) {
                    this.status = error;
                }
                cx.notify();
            }
        })
        .detach();
        let current_title = document
            .model()
            .map_or_else(String::new, |model| model.title.clone());
        editor.update(cx, |input, cx| input.set_value(&current_title, cx));
        let assets = document.load_assets();
        let asset_diagnostics = assets.diagnostics;
        let asset_images = load_asset_images(assets.images);
        Self {
            document,
            editor,
            root_focus: cx.focus_handle(),
            ime_geometry: ime_rect,
            current_slide: 0,
            outline_page: OutlinePage::default(),
            target: EditTarget::Title,
            canvas_editing: false,
            column_add_menu: None,
            presentation: None,
            presentation_slide: 0,
            return_slide: 0,
            presentation_cursor_hidden: false,
            cursor_hide_generation: 0,
            status: "KDL ファイルを開いて編集できます。".into(),
            pending_open: None,
            pending_close: false,
            pending_external_reload: false,
            save_as_pending: false,
            source_selection: None,
            layout_diagnostics: Vec::new(),
            asset_diagnostics,
            asset_images,
            presentation_assets: None,
            column_resize_drag: None,
            drop_destination: None,
        }
    }

    fn external_edit_blocked(&self) -> bool {
        self.pending_external_reload
            || !matches!(self.document.external_state(), ExternalState::Current)
    }

    fn has_uncommitted_draft(&self, cx: &Context<Self>) -> bool {
        self.editor.read(cx).is_marked()
            || self.editor.read(cx).value() != self.edit_value(self.target)
    }

    fn cancel_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.finish_composition(window, cx) {
            self.status = "日本語変換を終了できませんでした。".into();
            cx.notify();
            return;
        }
        let value = self.edit_value(self.target);
        self.editor
            .update(cx, |input, cx| input.set_value(&value, cx));
        window.focus(&self.root_focus, cx);
        self.status = "入力を取り消しました。".into();
        cx.notify();
    }

    fn poll_external_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked()
            || self.presentation.is_some()
            || self.document.path().is_none()
        {
            return;
        }
        let previous_slide = self.reload_slide_position();
        let has_draft = self.has_uncommitted_draft(cx);
        match self.document.synchronize_external(has_draft) {
            Ok(false) => return,
            Ok(true) => self.did_reload(previous_slide, window, cx),
            Err(_) => self.show_external_problem(window, cx),
        }
        cx.notify();
    }

    fn show_external_problem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.source_selection = None;
        self.status = match self.document.external_state() {
            ExternalState::Current => return,
            ExternalState::Conflict => "外部変更と未保存の編集が競合しています。編集を破棄して再読み込みするか、別名保存してください。".into(),
            ExternalState::Invalid(_) => "外部ファイルが無効です。表示中の内容を保持し、編集・保存・発表を停止しました。外部で修正して再読み込みしてください。".into(),
            ExternalState::Unavailable { kind: std::io::ErrorKind::NotFound, .. } => "外部ファイルが削除されています。編集中の内容を保持しています。ファイルを復元して再読み込みしてください。".into(),
            ExternalState::Unavailable { message, .. } => format!("外部ファイルを読み込めません。編集中の内容を保持しています: {message}"),
        };
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    fn edit_value(&self, target: EditTarget) -> String {
        let Some(model) = self.document.model() else {
            return String::new();
        };
        match target {
            EditTarget::Title => model.title.clone(),
            EditTarget::Columns { .. } => String::new(),
            EditTarget::ColumnWidth { slide, index, left } => model
                .slides
                .get(slide)
                .and_then(|slide| slide.elements.get(index))
                .and_then(|element| match element {
                    Element::Columns {
                        left_width,
                        right_width,
                        ..
                    } => Some(if left { *left_width } else { *right_width }.to_string()),
                    _ => None,
                })
                .unwrap_or_default(),
            EditTarget::Element {
                slide,
                index,
                field,
            } => model
                .slides
                .get(slide)
                .and_then(|slide| slide.elements.get(index))
                .map_or_else(String::new, |element| element_field_value(element, field)),
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                field,
            } => nested_element(&model, slide, columns, column, index)
                .map_or_else(String::new, |element| element_field_value(element, field)),
        }
    }

    fn select_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.target != EditTarget::Title {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = error;
                cx.notify();
                return;
            }
        } else {
            window.focus(&self.root_focus, cx);
            return;
        }
        self.target = EditTarget::Title;
        let value = self.edit_value(self.target);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&value, cx);
        });
        window.focus(&self.root_focus, cx);
    }

    fn select_element(
        &mut self,
        slide: usize,
        index: usize,
        field: ElementField,
        edit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = EditTarget::Element {
            slide,
            index,
            field,
        };
        if self.target == target {
            if self.canvas_editing && !edit {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
            let surface_changed = self.canvas_editing != edit;
            self.canvas_editing = edit;
            if edit {
                if surface_changed {
                    self.ime_geometry.refresh(window, true);
                }
                let focus = self.editor.read(cx).focus_handle();
                window.focus(&focus, cx);
            } else {
                window.focus(&self.root_focus, cx);
            }
            cx.notify();
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.current_slide = slide;
        self.target = target;
        self.canvas_editing = edit;
        self.column_add_menu = None;
        let value = self.edit_value(self.target);
        let multiline = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
            .and_then(|slide| slide.elements.get(index))
            .is_some_and(|element| match (element, field) {
                (
                    Element::Heading(_)
                    | Element::Text(_)
                    | Element::Code { .. }
                    | Element::Bullets(_),
                    ElementField::Text,
                ) => true,
                (Element::Image { .. }, ElementField::Caption) => true,
                _ => false,
            });
        self.editor.update(cx, |input, cx| {
            input.set_multiline(multiline, cx);
            input.set_value(&value, cx);
        });
        if edit {
            let focus = self.editor.read(cx).focus_handle();
            window.focus(&focus, cx);
        } else {
            window.focus(&self.root_focus, cx);
        }
        cx.notify();
    }

    fn select_nested_element(
        &mut self,
        slide: usize,
        columns: usize,
        column: usize,
        index: usize,
        field: ElementField,
        edit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = EditTarget::NestedElement {
            slide,
            columns,
            column,
            index,
            field,
        };
        if self.target == target {
            if self.canvas_editing && !edit {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
            let surface_changed = self.canvas_editing != edit;
            self.canvas_editing = edit;
            if edit {
                if surface_changed {
                    self.ime_geometry.refresh(window, true);
                }
                let focus = self.editor.read(cx).focus_handle();
                window.focus(&focus, cx);
            } else {
                window.focus(&self.root_focus, cx);
            }
            cx.notify();
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.current_slide = slide;
        self.target = target;
        self.canvas_editing = edit;
        self.column_add_menu = None;
        let value = self.edit_value(target);
        let multiline = self
            .document
            .model()
            .and_then(|model| nested_element(model, slide, columns, column, index))
            .is_some_and(|element| match (element, field) {
                (
                    Element::Heading(_)
                    | Element::Text(_)
                    | Element::Code { .. }
                    | Element::Bullets(_),
                    ElementField::Text,
                ) => true,
                (Element::Image { .. }, ElementField::Caption) => true,
                _ => false,
            });
        self.editor.update(cx, |input, cx| {
            input.set_multiline(multiline, cx);
            input.set_value(&value, cx);
        });
        if edit {
            let focus = self.editor.read(cx).focus_handle();
            window.focus(&focus, cx);
        } else {
            window.focus(&self.root_focus, cx);
        }
        cx.notify();
    }

    fn select_columns(
        &mut self,
        slide: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = EditTarget::Columns { slide, index };
        if self.target == target {
            window.focus(&self.root_focus, cx);
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.current_slide = slide;
        self.target = target;
        self.column_add_menu = None;
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value("", cx);
        });
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    fn select_column_width(
        &mut self,
        slide: usize,
        index: usize,
        left: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = EditTarget::ColumnWidth { slide, index, left };
        if self.target == target {
            let focus = self.editor.read(cx).focus_handle();
            window.focus(&focus, cx);
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.current_slide = slide;
        self.target = target;
        self.column_add_menu = None;
        let value = self.edit_value(target);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&value, cx);
        });
        let focus = self.editor.read(cx).focus_handle();
        window.focus(&focus, cx);
        cx.notify();
    }

    fn reset_editor_to_title(&mut self, cx: &mut Context<Self>) {
        self.target = EditTarget::Title;
        let value = self.edit_value(EditTarget::Title);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&value, cx);
        });
    }

    fn commit_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if self.external_edit_blocked() && !self.has_uncommitted_draft(cx) {
            return Ok(());
        }
        if self.editor.read(cx).is_marked() && !self.finish_composition(window, cx) {
            return Err("日本語変換を確定できませんでした。".into());
        }
        let value = self.editor.read(cx).value();
        let target = self.target;
        let history = self.editor.read(cx).history_states_for_commit();
        let redo = self.editor.read(cx).redo_states_for_commit();
        let baseline = self.editor.read(cx).history_baseline_for_commit();
        let mut validated = self.document.clone();
        apply_editor_value(&mut validated, target, &value).map_err(|error| error.to_string())?;
        if let Some(baseline) = baseline {
            // Input has already discarded older units. Start the shared
            // history at its surviving baseline, keeping the saved bytes.
            let _ = apply_editor_value(&mut self.document, target, &baseline);
            self.document.clear_history();
        }
        for state in history {
            if state != value {
                // Earlier drafts can be invalid while the user repairs a field
                // (for example an empty title). Keep valid editing units in the
                // shared history and skip states the document cannot represent.
                let _ = apply_editor_value(&mut self.document, target, &state);
            }
        }
        apply_editor_value(&mut self.document, target, &value)
            .map_err(|error| error.to_string())?;
        // Preserve undone input units as document Redo. Replay valid future
        // states, then return to the current state without saving any bytes.
        let mut future_units = 0;
        for state in redo {
            let before = self.document.source().to_owned();
            if apply_editor_value(&mut self.document, target, &state).is_ok()
                && self.document.source() != before
            {
                future_units += 1;
            }
        }
        for _ in 0..future_units {
            self.document.undo();
        }
        self.editor
            .update(cx, |input, _| input.reset_undo_history());
        self.refresh_assets();
        self.status = "編集内容を KDL に反映しました。".into();
        cx.notify();
        Ok(())
    }

    fn copy_input(&mut self, _: &input::Copy, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() || !self.editor.read(cx).has_focus(window) {
            return;
        }
        if let Some(text) = self.editor.read(cx).selected_text() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    fn cut_input(&mut self, _: &input::Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() || !self.editor.read(cx).has_focus(window) {
            return;
        }
        if !self.finish_composition(window, cx) {
            return;
        }
        if let Some(text) = self.editor.read(cx).selected_text() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            self.editor
                .update(cx, |input, cx| input.paste_text("", window, cx));
        }
    }

    fn paste_input(&mut self, _: &input::Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() || !self.editor.read(cx).has_focus(window) {
            return;
        }
        // Finalize through the native context before borrowing TextInputState,
        // so IME callbacks can update the input and its history safely.
        if !self.finish_composition(window, cx) {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.editor
                .update(cx, |input, cx| input.paste_text(&text, window, cx));
        }
    }

    fn commit_text_edit(
        &mut self,
        _: &CommitTextEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        match self.commit_draft(window, cx) {
            Ok(()) => {
                window.focus(&self.root_focus, cx);
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    #[allow(unexpected_cfgs)]
    fn finish_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.editor.read(cx).is_marked() {
            return true;
        }
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return false;
        };
        let view = handle.ns_view.as_ptr().cast::<Object>();
        let input_context: *mut Object = unsafe { msg_send![view, inputContext] };
        if input_context.is_null() {
            return false;
        }
        self.editor
            .update(cx, |input, cx| input.finish_composition(cx));
        unsafe {
            let _: () = msg_send![input_context, discardMarkedText];
        }
        !self.editor.read(cx).is_marked()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            self.status = "外部ファイルの問題を解決するか、別名保存してください。".into();
            cx.notify();
            return;
        }
        if self.document.path().is_none() {
            self.prompt_save_as(window, cx);
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("保存できませんでした: {error}");
            cx.notify();
            return;
        }
        window.focus(&self.root_focus, cx);
        match self.document.save() {
            Ok(()) => {
                self.source_selection = None;
                self.pending_external_reload = false;
                self.status = "保存しました。".into();
            }
            Err(error) => {
                self.show_external_problem(window, cx);
                self.status = format!("保存できませんでした: {error}");
            }
        }
        cx.notify();
    }

    fn save_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            self.status = "外部ファイルの問題を解決するか、別名保存してから閉じてください。".into();
            cx.notify();
            return;
        }
        if self.document.path().is_none() {
            self.prompt_save_as(window, cx);
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("保存できませんでした: {error}");
            cx.notify();
            return;
        }
        window.focus(&self.root_focus, cx);
        match self.document.save() {
            Ok(()) => {
                self.pending_close = false;
                window.remove_window();
            }
            Err(error) => {
                self.show_external_problem(window, cx);
                self.status = format!("保存できませんでした: {error}");
                cx.notify();
            }
        }
    }

    fn discard_and_close(&mut self, _: &MouseUpEvent, window: &mut Window, _: &mut Context<Self>) {
        self.pending_close = false;
        window.remove_window();
    }

    fn cancel_close(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pending_close = false;
        self.status = "閉じる操作を取り消しました。".into();
        cx.notify();
    }

    fn save_as(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_save_as(window, cx);
    }

    fn save_as_action(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_save_as(window, cx);
    }

    fn save_action(&mut self, _: &SaveFile, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    fn prompt_save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_as_pending {
            return;
        }
        self.save_as_pending = true;
        let notice = window.prompt(
            gpui::PromptLevel::Warning,
            "退避保存すると Undo / Redo 履歴が消去されます。",
            Some("別フォルダーでは、編集していない画像参照も更新されます。"),
            &["退避先を選ぶ", "キャンセル"],
            cx,
        );
        let view = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let proceed = matches!(notice.await, Ok(0));
                let _ = cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        if proceed {
                            this.choose_save_as_path(window, cx);
                        } else {
                            this.save_as_pending = false;
                            cx.notify();
                        }
                    })
                });
            })
            .detach();
    }

    fn choose_save_as_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = self
            .document
            .path()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        let receiver = cx.prompt_for_new_path(directory, Some("presentation.kdl"));
        let view = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let selected = receiver.await;
                let _ = cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        this.save_as_pending = false;
                        cx.notify();
                    })
                });
                let Ok(Ok(Some(path))) = selected else {
                    return;
                };
                let _ = cx.update(|window, cx| {
                    let close_after_save = view.update(cx, |this, cx| {
                        let result = this.save_as_with_draft(&path, window, cx);
                        if result.is_ok() {
                            this.refresh_assets();
                            // Save As can rebase the selected image path. Keep
                            // the field in sync so the next Save cannot restore
                            // the previous document's relative path.
                            let value = this.edit_value(this.target);
                            this.editor
                                .update(cx, |input, cx| input.set_value(&value, cx));
                            this.source_selection = None;
                            this.pending_external_reload = false;
                        }
                        let close_after_save = this.pending_close && result.is_ok();
                        this.status = result.map_or_else(
                            |error| format!("退避保存できませんでした: {error}"),
                            |_| "別ファイルへ保存しました。".into(),
                        );
                        if close_after_save {
                            this.pending_close = false;
                        }
                        cx.notify();
                        close_after_save
                    });
                    if close_after_save {
                        window.remove_window();
                    }
                });
            })
            .detach();
    }

    fn save_as_with_draft(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.editor.read(cx).is_marked() && !self.finish_composition(window, cx) {
            return Err("日本語変換を確定できませんでした。".into());
        }
        let mut candidate = PresentationDocument::from_source_with_asset_base(
            self.document.source(),
            self.document.asset_base(),
        )
        .map_err(|error| error.to_string())?;
        apply_editor_value(&mut candidate, self.target, &self.editor.read(cx).value())
            .map_err(|error| error.to_string())?;
        self.document
            .save_as_source_overwriting(path, candidate.source())
            .map_err(|error| error.to_string())?;
        window.focus(&self.root_focus, cx);
        Ok(())
    }

    fn open_picker(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("KDL Presentation を開く".into()),
        });
        let view = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let Ok(Ok(Some(paths))) = receiver.await else {
                    return;
                };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let _ = cx.update(|window, cx| {
                    view.update(cx, |this, cx| this.request_open(path, window, cx));
                });
            })
            .detach();
    }

    fn request_open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("編集内容を確定できないため開けません: {error}");
            cx.notify();
            return;
        }
        if self.document.is_dirty() {
            self.pending_open = Some(path);
            self.status =
                "未保存の編集があります。保存して開くか、破棄して開くか選んでください。".into();
            cx.notify();
        } else {
            self.open_document(path, window, cx);
        }
    }

    fn open_document(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match PresentationDocument::open(&path) {
            Ok(document) => {
                self.document = document;
                self.reset_document_view(window, cx);
                self.status = format!(
                    "{} を開きました。",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(error) => self.status = format!("ファイルを開けませんでした: {error}"),
        }
        self.pending_open = None;
        cx.notify();
    }

    fn reset_document_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_assets();
        self.current_slide = 0;
        self.target = EditTarget::Title;
        self.source_selection = None;
        self.pending_external_reload = false;
        self.column_resize_drag = None;
        self.drop_destination = None;
        let title = self.edit_value(EditTarget::Title);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&title, cx);
        });
        window.focus(&self.root_focus, cx);
    }

    fn reload_slide_position(&self) -> (usize, Option<String>) {
        let id = self
            .document
            .model()
            .and_then(|model| model.slides.get(self.current_slide))
            .and_then(|slide| slide.id.clone());
        (self.current_slide, id)
    }

    fn did_reload(
        &mut self,
        (previous_index, previous_id): (usize, Option<String>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reset_document_view(window, cx);
        if let Some(model) = self.document.model() {
            self.current_slide = previous_id
                .as_ref()
                .and_then(|id| {
                    model
                        .slides
                        .iter()
                        .position(|slide| slide.id.as_ref() == Some(id))
                })
                .unwrap_or(previous_index.min(model.slides.len().saturating_sub(1)));
        }
        self.status = "外部ファイルを再読み込みしました。".into();
    }

    fn reload_external_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_external_reload
            && (self.document.is_dirty() || self.has_uncommitted_draft(cx))
        {
            self.pending_external_reload = true;
            self.status =
                "再読み込みすると未保存の編集内容を破棄します。続行するか、別名保存してください。"
                    .into();
            cx.notify();
            return;
        }
        let previous_slide = self.reload_slide_position();
        match self.document.reload_from_disk() {
            Ok(()) => self.did_reload(previous_slide, window, cx),
            Err(_) => {
                self.pending_external_reload = false;
                self.show_external_problem(window, cx);
            }
        }
        cx.notify();
    }

    fn resolve_pending_open(
        &mut self,
        save_first: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.pending_open.clone() else {
            return;
        };
        if save_first {
            if let Err(error) = self
                .commit_draft(window, cx)
                .and_then(|()| self.document.save().map_err(|error| error.to_string()))
            {
                self.status = format!("保存できませんでした: {error}");
                cx.notify();
                return;
            }
        }
        self.open_document(path, window, cx);
    }

    fn add_slide(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .add_slide()
                .map(|index| self.current_slide = index)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("Slide を追加できませんでした: {error}");
        } else {
            self.reset_editor_to_title(cx);
            window.focus(&self.root_focus, cx);
            self.status = "Slide を追加しました。".into();
        }
        cx.notify();
    }

    fn delete_slide(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .remove_slide(self.current_slide)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("Slide を削除できませんでした: {error}");
        } else {
            let count = self.document.model().map_or(0, |model| model.slides.len());
            self.current_slide = self.current_slide.min(count.saturating_sub(1));
            self.reset_editor_to_title(cx);
            window.focus(&self.root_focus, cx);
            self.status = "Slide を削除しました。".into();
        }
        cx.notify();
    }

    fn add_element(&mut self, kind: ElementKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        if self.commit_draft(window, cx).is_err() {
            return;
        }
        match self.document.add_element(self.current_slide, kind) {
            Ok(index) => {
                if kind == ElementKind::Columns {
                    self.select_columns(self.current_slide, index, window, cx);
                } else {
                    self.select_element(
                        self.current_slide,
                        index,
                        if kind == ElementKind::Image {
                            ElementField::ImagePath
                        } else {
                            ElementField::Text
                        },
                        true,
                        window,
                        cx,
                    );
                }
                self.status = "Element を追加しました。".into();
            }
            Err(error) => self.status = format!("Element を追加できませんでした: {error}"),
        }
        cx.notify();
    }

    fn add_column_element_selection(
        &mut self,
        column: usize,
        kind: ElementKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        let EditTarget::Columns {
            slide,
            index: columns,
        } = self.target
        else {
            return;
        };
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        match self
            .document
            .add_column_element(slide, columns, column, kind)
        {
            Ok(index) => {
                self.column_add_menu = None;
                let field = if kind == ElementKind::Image {
                    ElementField::ImagePath
                } else {
                    ElementField::Text
                };
                self.select_nested_element(slide, columns, column, index, field, true, window, cx);
                self.status = "列に Element を追加しました。".into();
            }
            Err(error) => self.status = format!("列に Element を追加できませんでした: {error}"),
        }
        cx.notify();
    }

    fn delete_element(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let target = self.target;
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("編集を確定できないため削除できません: {error}");
            cx.notify();
            return;
        }
        let result = match target {
            EditTarget::Element { slide, index, .. }
            | EditTarget::Columns { slide, index }
            | EditTarget::ColumnWidth { slide, index, .. } => {
                self.document.remove_element(slide, index)
            }
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => self
                .document
                .remove_column_element(slide, columns, column, index),
            EditTarget::Title => {
                self.status = "削除する Element を選択してください。".into();
                cx.notify();
                return;
            }
        };
        match result {
            Ok(()) => {
                self.reset_editor_to_title(cx);
                window.focus(&self.root_focus, cx);
                self.status = "Element を削除しました。".into();
            }
            Err(error) => self.status = format!("Element を削除できませんでした: {error}"),
        }
        cx.notify();
    }

    fn move_slide_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let count = self.document.model().map_or(0, |model| model.slides.len());
        if count == 0 {
            return;
        }
        let from = self.current_slide;
        let to = (from as isize + delta).clamp(0, count as isize - 1) as usize;
        if from == to {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .move_slide(from, to)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("Slide を移動できませんでした: {error}");
            cx.notify();
            return;
        }
        self.current_slide = to;
        // The draft was committed before moving. Its numeric target now
        // refers to another slide, so selection must not commit it again.
        self.reset_editor_to_title(cx);
        window.focus(&self.root_focus, cx);
        self.status = "Slide の順序を変更しました。".into();
        cx.notify();
    }

    fn move_element_selection(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        let (slide, from, field) = match self.target {
            EditTarget::Element {
                slide,
                index,
                field,
            } => (slide, index, Some(field)),
            EditTarget::Columns { slide, index } | EditTarget::ColumnWidth { slide, index, .. } => {
                (slide, index, None)
            }
            EditTarget::NestedElement { .. } | EditTarget::Title => return,
        };
        let count = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
            .map_or(0, |slide| slide.elements.len());
        if count == 0 {
            return;
        }
        let to = (from as isize + delta).clamp(0, count as isize - 1) as usize;
        if from == to {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .move_element(slide, from, to)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("Element を移動できませんでした: {error}");
            cx.notify();
            return;
        }
        self.target = if let Some(field) = field {
            EditTarget::Element {
                slide,
                index: to,
                field,
            }
        } else {
            EditTarget::Columns { slide, index: to }
        };
        self.refresh_editor_value(cx);
        window.focus(&self.root_focus, cx);
        self.status = "Element の順序を変更しました。".into();
        cx.notify();
    }

    fn move_nested_element_selection(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        let EditTarget::NestedElement {
            slide,
            columns,
            column,
            index: from,
            field,
        } = self.target
        else {
            return;
        };
        let count = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
            .and_then(|slide| slide.elements.get(columns))
            .and_then(|element| match element {
                Element::Columns { left, right, .. } => {
                    Some(if column == 0 { left } else { right })
                }
                _ => None,
            })
            .map_or(0, Vec::len);
        if count == 0 {
            return;
        }
        let to = (from as isize + delta).clamp(0, count as isize - 1) as usize;
        if from == to {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .move_column_element(slide, columns, column, from, to)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("列内の Element を移動できませんでした: {error}");
        } else {
            self.target = EditTarget::NestedElement {
                slide,
                columns,
                column,
                index: to,
                field,
            };
            self.refresh_editor_value(cx);
            window.focus(&self.root_focus, cx);
            self.status = "列内の Element の順序を変更しました。".into();
        }
        cx.notify();
    }

    fn move_top_level_element_to_column(
        &mut self,
        columns: usize,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        let EditTarget::Element {
            slide,
            index,
            field,
        } = self.target
        else {
            return;
        };
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        match self
            .document
            .move_element_to_column(slide, index, columns, column)
        {
            Ok((columns, index)) => {
                self.target = EditTarget::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                    field,
                };
                self.refresh_editor_value(cx);
                window.focus(&self.root_focus, cx);
                self.status = "Element を列へ移動しました。".into();
            }
            Err(error) => self.status = format!("Element を列へ移動できませんでした: {error}"),
        }
        cx.notify();
    }

    fn move_nested_element_to_slide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let EditTarget::NestedElement {
            slide,
            columns,
            column,
            index,
            field,
        } = self.target
        else {
            return;
        };
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        match self
            .document
            .move_column_element_to_slide(slide, columns, column, index)
        {
            Ok(index) => {
                self.target = EditTarget::Element {
                    slide,
                    index,
                    field,
                };
                self.refresh_editor_value(cx);
                window.focus(&self.root_focus, cx);
                self.status = "Element を Slide 直下へ移動しました。".into();
            }
            Err(error) => self.status = format!("Element を移動できませんでした: {error}"),
        }
        cx.notify();
    }

    fn bullet_source(&self) -> Option<CanvasSource> {
        match self.target {
            EditTarget::Element { slide, index, .. } => {
                Some(CanvasSource::Element { slide, index })
            }
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => Some(CanvasSource::NestedElement {
                slide,
                columns,
                column,
                index,
            }),
            _ => None,
        }
    }

    fn bullet_target(&self) -> Option<(CanvasSource, usize)> {
        match self.target {
            EditTarget::Element {
                field: ElementField::Bullet(item),
                ..
            }
            | EditTarget::NestedElement {
                field: ElementField::Bullet(item),
                ..
            } => Some((self.bullet_source()?, item)),
            _ => None,
        }
    }

    fn bullet_items(&self, source: CanvasSource) -> Option<&[String]> {
        let model = self.document.model()?;
        let element = match source {
            CanvasSource::Element { slide, index } => {
                model.slides.get(slide)?.elements.get(index)?
            }
            CanvasSource::NestedElement {
                slide,
                columns,
                column,
                index,
            } => nested_element(model, slide, columns, column, index)?,
        };
        match element {
            Element::Bullets(items) => Some(items),
            _ => None,
        }
    }

    fn select_bullet_field(
        &mut self,
        source: CanvasSource,
        field: ElementField,
        edit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match source {
            CanvasSource::Element { slide, index } => {
                self.select_element(slide, index, field, edit, window, cx)
            }
            CanvasSource::NestedElement {
                slide,
                columns,
                column,
                index,
            } => self.select_nested_element(slide, columns, column, index, field, edit, window, cx),
        }
    }

    fn add_bullet_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let Some(source) = self.bullet_source() else {
            return;
        };
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("項目を追加できませんでした: {error}");
            cx.notify();
            return;
        }
        let Some(insertion) = self.bullet_items(source).map(<[String]>::len) else {
            return;
        };
        let result = match source {
            CanvasSource::Element { slide, index } => {
                self.document.add_bullet(slide, index, insertion, "")
            }
            CanvasSource::NestedElement {
                slide,
                columns,
                column,
                index,
            } => self
                .document
                .add_column_bullet(slide, columns, column, index, insertion, ""),
        };
        if let Err(error) = result {
            self.status = format!("項目を追加できませんでした: {error}");
        } else {
            // Adding changes the whole-list field. Do not commit its old
            // contents again while selecting the newly inserted item.
            self.reset_editor_to_title(cx);
            self.select_bullet_field(source, ElementField::Bullet(insertion), true, window, cx);
            self.status = "箇条書き項目を追加しました。".into();
        }
        cx.notify();
    }

    fn remove_bullet_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let Some((source, item)) = self.bullet_target() else {
            return;
        };
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            match source {
                CanvasSource::Element { slide, index } => {
                    self.document.remove_bullet(slide, index, item)
                }
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                } => self
                    .document
                    .remove_column_bullet(slide, columns, column, index, item),
            }
            .map_err(|error| error.to_string())
        }) {
            self.status = format!("項目を削除できませんでした: {error}");
        } else {
            self.reset_editor_to_title(cx);
            let count = self.bullet_items(source).map_or(0, <[String]>::len);
            if count > 0 {
                self.select_bullet_field(
                    source,
                    ElementField::Bullet(item.min(count - 1)),
                    true,
                    window,
                    cx,
                );
            } else {
                self.select_bullet_field(source, ElementField::Text, false, window, cx);
            }
            self.status = "箇条書き項目を削除しました。".into();
        }
        cx.notify();
    }

    fn move_bullet_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let Some((source, from)) = self.bullet_target() else {
            return;
        };
        let count = self.bullet_items(source).map_or(0, <[String]>::len);
        if count == 0 {
            return;
        }
        let to = (from as isize + delta).clamp(0, count as isize - 1) as usize;
        if from == to {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            match source {
                CanvasSource::Element { slide, index } => {
                    self.document.move_bullet(slide, index, from, to)
                }
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                } => self
                    .document
                    .move_column_bullet(slide, columns, column, index, from, to),
            }
            .map_err(|error| error.to_string())
        }) {
            self.status = format!("項目を移動できませんでした: {error}");
        } else {
            self.target = match source {
                CanvasSource::Element { slide, index } => EditTarget::Element {
                    slide,
                    index,
                    field: ElementField::Bullet(to),
                },
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                } => EditTarget::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                    field: ElementField::Bullet(to),
                },
            };
            self.refresh_editor_value(cx);
            window.focus(&self.root_focus, cx);
            self.status = "箇条書き項目の順序を変更しました。".into();
        }
        cx.notify();
    }

    fn undo_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            self.status = "外部ファイルの競合を解決するまで Undo は使えません。".into();
            cx.notify();
            return;
        }
        if self.editor.read(cx).has_focus(window) {
            window.dispatch_action(Box::new(input::Undo), cx);
            return;
        }
        if self.has_uncommitted_draft(cx) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = format!("入力中の編集を確定できません: {error}");
                cx.notify();
                return;
            }
        }
        let previous_model = self.document.model().cloned();
        let active_slide_id = self.current_slide_id();
        if self.document.undo() {
            self.status = "元に戻しました。".into();
            self.restore_editor_after_history(previous_model.as_ref(), active_slide_id, window, cx);
        } else {
            self.status = "戻せる編集はありません。".into();
            self.refresh_editor_value(cx);
        }
        cx.notify();
    }

    fn redo_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            self.status = "外部ファイルの競合を解決するまで Redo は使えません。".into();
            cx.notify();
            return;
        }
        if self.editor.read(cx).has_focus(window) {
            window.dispatch_action(Box::new(input::Redo), cx);
            return;
        }
        if self.has_uncommitted_draft(cx) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = format!("入力中の編集を確定できません: {error}");
                cx.notify();
                return;
            }
        }
        let previous_model = self.document.model().cloned();
        let active_slide_id = self.current_slide_id();
        if self.document.redo() {
            self.status = "やり直しました。".into();
            self.restore_editor_after_history(previous_model.as_ref(), active_slide_id, window, cx);
        } else {
            self.status = "やり直せる編集はありません。".into();
            self.refresh_editor_value(cx);
        }
        cx.notify();
    }

    fn current_slide_id(&self) -> Option<String> {
        self.document
            .model()
            .and_then(|model| model.slides.get(self.current_slide))
            .and_then(|slide| slide.id.clone())
    }

    fn restore_editor_after_history(
        &mut self,
        previous_model: Option<&PresentationModel>,
        active_slide_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut target_was_invalid = !self.target_is_valid(self.target);
        self.current_slide =
            self.slide_restored_by_history(previous_model, active_slide_id.clone());
        target_was_invalid |=
            target_slide(self.target).is_some_and(|slide| slide != self.current_slide);
        if target_was_invalid {
            self.target = EditTarget::Title;
        }
        if (target_was_invalid || matches!(self.target, EditTarget::Title))
            && let Some(restored_target) = previous_model.and_then(|previous| {
                let model = self.document.model()?;
                let current = model.slides.get(self.current_slide)?;
                let before = if let Some(id) = &current.id {
                    previous
                        .slides
                        .iter()
                        .find(|slide| slide.id.as_ref() == Some(id))?
                } else {
                    previous
                        .slides
                        .iter()
                        .find(|slide| *slide == current)
                        .or_else(|| {
                            (previous.slides.len() == model.slides.len())
                                .then(|| previous.slides.get(self.current_slide))
                                .flatten()
                        })?
                };
                restored_element_after_history(before, current, self.current_slide)
            })
        {
            self.target = match restored_target {
                CanvasSource::Element { slide, index } => {
                    let element = self
                        .document
                        .model()
                        .and_then(|model| model.slides.get(slide))
                        .and_then(|slide| slide.elements.get(index));
                    if element.is_some_and(|element| matches!(element, Element::Columns { .. })) {
                        EditTarget::Columns { slide, index }
                    } else {
                        EditTarget::Element {
                            slide,
                            index,
                            field: element.map_or(ElementField::Text, default_edit_field),
                        }
                    }
                }
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                } => {
                    let element = self
                        .document
                        .model()
                        .and_then(|model| nested_element(model, slide, columns, column, index));
                    EditTarget::NestedElement {
                        slide,
                        columns,
                        column,
                        index,
                        field: element.map_or(ElementField::Text, default_edit_field),
                    }
                }
            };
            window.focus(&self.root_focus, cx);
        }
        self.column_add_menu = None;
        self.refresh_assets();
        self.refresh_editor_value(cx);
    }

    fn slide_restored_by_history(
        &self,
        previous_model: Option<&PresentationModel>,
        active_slide_id: Option<String>,
    ) -> usize {
        let Some(model) = self.document.model() else {
            return 0;
        };
        if let Some(previous_model) = previous_model {
            if model.slides.len() > previous_model.slides.len() {
                if let Some(index) = model.slides.iter().position(|slide| {
                    slide.id.as_ref().is_some_and(|id| {
                        !previous_model
                            .slides
                            .iter()
                            .any(|previous| previous.id.as_ref() == Some(id))
                    })
                }) {
                    return index;
                }
                if let Some(index) = model
                    .slides
                    .iter()
                    .zip(previous_model.slides.iter())
                    .position(|(after, before)| after != before)
                {
                    return index;
                }
                return model.slides.len() - 1;
            }
            if previous_model.slides.len() == model.slides.len()
                && !slides_have_same_members(&previous_model.slides, &model.slides)
                && let Some(index) = previous_model
                    .slides
                    .iter()
                    .zip(&model.slides)
                    .position(|(before, after)| before != after)
            {
                // A content edit belongs to its changed Slide, even if the
                // user navigated elsewhere before invoking Undo or Redo.
                return index;
            }
        }
        if let Some(active_slide_id) = active_slide_id {
            if let Some(index) = model
                .slides
                .iter()
                .position(|slide| slide.id.as_ref() == Some(&active_slide_id))
            {
                return index;
            }
        } else if let Some(previous) = previous_model
            .filter(|previous| previous.slides.len() == model.slides.len())
            .and_then(|previous| previous.slides.get(self.current_slide))
        {
            // Slides without IDs can still be followed through a reorder
            // when their complete contents uniquely identify them.
            let mut matching = model
                .slides
                .iter()
                .enumerate()
                .filter(|(_, slide)| *slide == previous);
            if let Some((index, _)) = matching.next()
                && matching.next().is_none()
            {
                return index;
            }
        }
        target_slide(self.target)
            .unwrap_or(self.current_slide)
            .min(model.slides.len().saturating_sub(1))
    }

    fn target_is_valid(&self, target: EditTarget) -> bool {
        let Some(model) = self.document.model() else {
            return false;
        };
        match target {
            EditTarget::Title => true,
            EditTarget::Element {
                slide,
                index,
                field,
            } => model
                .slides
                .get(slide)
                .and_then(|slide| slide.elements.get(index))
                .is_some_and(|element| element_supports_field(element, field)),
            EditTarget::Columns { slide, index } | EditTarget::ColumnWidth { slide, index, .. } => {
                model
                    .slides
                    .get(slide)
                    .and_then(|slide| slide.elements.get(index))
                    .is_some_and(|element| matches!(element, Element::Columns { .. }))
            }
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                field,
            } => nested_element(model, slide, columns, column, index)
                .is_some_and(|element| element_supports_field(element, field)),
        }
    }

    fn undo(&mut self, _: &UndoEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.undo_changes(window, cx);
    }

    fn undo_button(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.undo_changes(window, cx);
    }

    fn redo(&mut self, _: &RedoEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.redo_changes(window, cx);
    }

    fn redo_button(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.redo_changes(window, cx);
    }

    fn refresh_editor_value(&mut self, cx: &mut Context<Self>) {
        let value = self.edit_value(self.target);
        self.editor
            .update(cx, |input, cx| input.set_value(&value, cx));
    }

    fn refresh_assets(&mut self) {
        let assets = self.document.load_assets();
        self.asset_diagnostics = assets.diagnostics;
        self.asset_images = load_asset_images(assets.images);
    }

    fn navigate_to_diagnostic(
        &mut self,
        diagnostic: Diagnostic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.source_selection = Some(diagnostic.clone());
        cx.notify();
        let Some(model) = self.document.model() else {
            return;
        };
        let slide_index = diagnostic
            .slide_id
            .as_ref()
            .and_then(|id| {
                model
                    .slides
                    .iter()
                    .position(|slide| slide.id.as_ref() == Some(id))
            })
            .or_else(|| {
                diagnostic
                    .slide_index
                    .filter(|index| *index < model.slides.len())
            });
        let Some(slide_index) = slide_index else {
            return;
        };
        if let Some(element_index) = diagnostic.element_index {
            if let Some(column_index) = diagnostic.column_index {
                if let Some(nested_index) = diagnostic.nested_element_index {
                    if let Some(element) = nested_element(
                        model,
                        slide_index,
                        element_index,
                        column_index,
                        nested_index,
                    ) {
                        let field = default_edit_field(element);
                        self.select_nested_element(
                            slide_index,
                            element_index,
                            column_index,
                            nested_index,
                            field,
                            false,
                            window,
                            cx,
                        );
                        return;
                    }
                }
            } else if let Some(element) = model
                .slides
                .get(slide_index)
                .and_then(|slide| slide.elements.get(element_index))
            {
                if matches!(element, Element::Columns { .. }) {
                    self.select_columns(slide_index, element_index, window, cx);
                } else {
                    self.select_element(
                        slide_index,
                        element_index,
                        default_edit_field(element),
                        false,
                        window,
                        cx,
                    );
                }
                return;
            }
        }
        self.current_slide = slide_index;
        self.reset_editor_to_title(cx);
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    fn navigate_to_asset(
        &mut self,
        issue: AssetDiagnostic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target_exists = self.document.model().is_some_and(|model| {
            if let (Some(column), Some(nested)) = (issue.column_index, issue.nested_element_index) {
                nested_element(
                    model,
                    issue.slide_index,
                    issue.element_index,
                    column,
                    nested,
                )
                .is_some_and(|element| matches!(element, Element::Image { .. }))
            } else {
                model
                    .slides
                    .get(issue.slide_index)
                    .and_then(|slide| slide.elements.get(issue.element_index))
                    .is_some_and(|element| matches!(element, Element::Image { .. }))
            }
        });
        if !target_exists {
            return;
        }
        if let (Some(column), Some(nested)) = (issue.column_index, issue.nested_element_index) {
            self.select_nested_element(
                issue.slide_index,
                issue.element_index,
                column,
                nested,
                ElementField::ImagePath,
                false,
                window,
                cx,
            );
        } else {
            self.select_element(
                issue.slide_index,
                issue.element_index,
                ElementField::ImagePath,
                false,
                window,
                cx,
            );
        }
        self.status = format!("画像を読み込めません: {} · {}", issue.path, issue.problem);
        cx.notify();
    }

    fn pointer_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.presentation.is_none() {
            return;
        }
        let cursor_was_hidden = self.presentation_cursor_hidden;
        self.show_presentation_cursor();
        self.schedule_cursor_hide(window, cx);
        if cursor_was_hidden {
            cx.notify();
        }
    }

    fn schedule_cursor_hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cursor_hide_generation = self.cursor_hide_generation.wrapping_add(1);
        let generation = self.cursor_hide_generation;
        let view = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let _ = cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        if this.presentation.is_some() && this.cursor_hide_generation == generation
                        {
                            this.hide_presentation_cursor();
                            cx.notify();
                        }
                    })
                });
            })
            .detach();
    }

    fn hide_presentation_cursor(&mut self) {
        if self.presentation_cursor_hidden {
            return;
        }
        unsafe {
            let _: () = msg_send![objc::class!(NSCursor), hide];
        }
        self.presentation_cursor_hidden = true;
    }

    fn show_presentation_cursor(&mut self) {
        if !self.presentation_cursor_hidden {
            return;
        }
        unsafe {
            let _: () = msg_send![objc::class!(NSCursor), unhide];
        }
        self.presentation_cursor_hidden = false;
        self.cursor_hide_generation = self.cursor_hide_generation.wrapping_add(1);
    }

    fn start_presentation(
        &mut self,
        start_at_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            self.status = "外部ファイルの問題を解決してから発表してください。".into();
            cx.notify();
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = format!("発表を開始できません: {error}");
            cx.notify();
            return;
        }
        self.poll_external_change(window, cx);
        if self.external_edit_blocked() {
            return;
        }
        let Some(model) = self.document.model().cloned() else {
            self.status = "KDL または Schema の診断を修正してから発表してください。".into();
            cx.notify();
            return;
        };
        if model.slides.is_empty() {
            self.status = "Slide がないため発表を開始できません。".into();
            cx.notify();
            return;
        }
        self.layout_diagnostics = collect_layout_diagnostics(&model, window);
        if let Some(issue) = self.layout_diagnostics.first() {
            self.status = format!(
                "発表を開始できません: Slide {}、Element {} の配置がスライドからはみ出します。",
                issue.slide_index + 1,
                issue.element_index + 1
            );
            cx.notify();
            return;
        }
        self.refresh_assets();
        if let Some(issue) = self.asset_diagnostics.first() {
            self.status = format!(
                "発表を開始できません: Slide {} の画像 {}: {}",
                issue.slide_index + 1,
                issue.path,
                issue.problem
            );
            cx.notify();
            return;
        }
        self.return_slide = self.current_slide.min(model.slides.len() - 1);
        self.presentation_slide = if start_at_current {
            self.return_slide
        } else {
            0
        };
        self.presentation = Some(model);
        self.presentation_assets = Some(self.asset_images.clone());
        self.status = "発表中です。Escape で編集へ戻ります。".into();
        window.toggle_fullscreen();
        self.schedule_cursor_hide(window, cx);
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    fn next_slide(&mut self, _: &NextSlide, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(model) = self.presentation.as_ref() {
            self.presentation_slide =
                (self.presentation_slide + 1).min(model.slides.len().saturating_sub(1));
        }
        cx.notify();
    }

    fn previous_slide(&mut self, _: &PreviousSlide, _: &mut Window, cx: &mut Context<Self>) {
        self.presentation_slide = self.presentation_slide.saturating_sub(1);
        cx.notify();
    }

    fn exit_presentation(
        &mut self,
        _: &ExitPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.presentation.is_none() {
            return;
        }
        self.return_slide = self.presentation_slide;
        self.current_slide = self.return_slide;
        self.presentation = None;
        self.presentation_assets = None;
        self.show_presentation_cursor();
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        window.focus(&self.root_focus, cx);
        self.refresh_assets();
        self.status = "発表を終了しました。".into();
        self.poll_external_change(window, cx);
        cx.notify();
    }

    fn preview_model(&self, cx: &App) -> Option<PresentationModel> {
        let value = self.editor.read(cx).value();
        if self.external_edit_blocked() || value == self.edit_value(self.target) {
            return self.document.model().cloned();
        }
        // Preview the draft without adding history entries or changing saved bytes.
        let mut preview = PresentationDocument::from_source_with_asset_base(
            self.document.source(),
            self.document.asset_base(),
        )
        .ok()?;
        if apply_editor_value(&mut preview, self.target, &value).is_ok() {
            preview.model().cloned()
        } else {
            self.document.model().cloned()
        }
    }

    fn render_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.editor
            .update(cx, |input, _| input.set_canvas_layout(None));
        let model = self.preview_model(cx);
        let editing_available = !self.external_edit_blocked();
        let input = self.editor.read(cx);
        let input_focused = input.has_focus(window);
        let can_undo = if input_focused {
            input.can_undo()
        } else {
            self.document.can_undo()
        };
        let can_redo = if input_focused {
            input.can_redo()
        } else {
            self.document.can_redo()
        };
        self.layout_diagnostics = model
            .as_ref()
            .map(|model| collect_layout_diagnostics(model, window))
            .unwrap_or_default();
        let layout_valid = self.layout_diagnostics.is_empty();
        let title = model
            .as_ref()
            .map_or("新しい Presentation", |model| model.title.as_str())
            .to_owned();
        let mut toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(0xf0f1f3));
        toolbar = toolbar
            .child(button(cx, "開く", true, Self::open_picker))
            .child(button(
                cx,
                "保存",
                model.is_some() && !self.external_edit_blocked(),
                |this, _, window, cx| this.save(window, cx),
            ))
            .child(button(cx, "別名保存", model.is_some(), Self::save_as))
            .child(button(
                cx,
                "Undo",
                can_undo && editing_available,
                Self::undo_button,
            ))
            .child(button(
                cx,
                "Redo",
                can_redo && editing_available,
                Self::redo_button,
            ))
            .child(button(cx, "＋ Slide", editing_available, Self::add_slide))
            .child(button(
                cx,
                "− Slide",
                editing_available && model.as_ref().is_some_and(|model| !model.slides.is_empty()),
                Self::delete_slide,
            ))
            .child(button(
                cx,
                "Slide ↑",
                editing_available && self.current_slide > 0,
                |this, _, window, cx| this.move_slide_selection(-1, window, cx),
            ))
            .child(button(
                cx,
                "Slide ↓",
                model
                    .as_ref()
                    .is_some_and(|model| self.current_slide + 1 < model.slides.len())
                    && editing_available,
                |this, _, window, cx| this.move_slide_selection(1, window, cx),
            ))
            .child(button(
                cx,
                "最初から発表",
                model.as_ref().is_some_and(|model| !model.slides.is_empty())
                    && editing_available
                    && !self.external_edit_blocked()
                    && layout_valid,
                |this, _, window, cx| this.start_presentation(false, window, cx),
            ))
            .child(button(
                cx,
                "現在から発表",
                model.as_ref().is_some_and(|model| !model.slides.is_empty())
                    && editing_available
                    && !self.external_edit_blocked()
                    && layout_valid,
                |this, _, window, cx| this.start_presentation(true, window, cx),
            ));
        if self.pending_open.is_some() {
            toolbar = toolbar
                .child(button(
                    cx,
                    "保存して開く",
                    true,
                    |this, _, window, cx| this.resolve_pending_open(true, window, cx),
                ))
                .child(button(
                    cx,
                    "破棄して開く",
                    true,
                    |this, _, window, cx| this.resolve_pending_open(false, window, cx),
                ))
                .child(button(cx, "キャンセル", true, |this, _, _, cx| {
                    this.pending_open = None;
                    this.status = "ファイルを開く操作を取り消しました。".into();
                    cx.notify();
                }));
        }
        if self.pending_close {
            toolbar = toolbar
                .child(button(
                    cx,
                    "保存して閉じる",
                    true,
                    |this, _, window, cx| this.save_and_close(window, cx),
                ))
                .child(button(cx, "破棄して閉じる", true, Self::discard_and_close))
                .child(button(cx, "閉じるのをやめる", true, Self::cancel_close));
        }
        if self.external_edit_blocked() {
            if self.pending_external_reload {
                toolbar = toolbar
                    .child(button(cx, "編集を破棄して再読み込み", true, |this, _, window, cx| {
                        this.reload_external_change(window, cx)
                    }))
                    .child(button(cx, "読み込みをやめる", true, |this, _, _, cx| {
                        this.pending_external_reload = false;
                        this.status = "外部変更の読み込みを取り消しました。別名保存で編集を退避できます。".into();
                        cx.notify();
                    }));
            } else {
                toolbar = toolbar.child(button(
                    cx,
                    "外部変更を読み込む",
                    true,
                    |this, _, window, cx| this.reload_external_change(window, cx),
                ));
            }
        }

        let side = self.render_slide_list(model.as_ref(), cx);
        let canvas = if let Some(model) = model.as_ref() {
            self.render_canvas(model, self.current_slide, true, window, cx)
        } else {
            diagnostics_view(
                self.document.diagnostics(),
                self.source_selection.as_ref(),
                cx,
            )
            .into_any_element()
        };
        let inspector = if let ExternalState::Invalid(diagnostics) = self.document.external_state()
        {
            div()
                .id("external-diagnostics")
                .w(px(270.0))
                .flex_shrink_0()
                .h_full()
                .overflow_y_scroll()
                .child(diagnostics_view(
                    diagnostics,
                    self.source_selection.as_ref(),
                    cx,
                ))
                .into_any_element()
        } else if let ExternalState::Unavailable { message: error, .. } =
            self.document.external_state()
        {
            div()
                .flex()
                .flex_col()
                .w(px(270.0))
                .p_3()
                .text_color(rgb(0x991b1b))
                .child("外部ファイルを読み込めません")
                .child(error.clone())
                .into_any_element()
        } else {
            self.render_inspector(model.as_ref(), cx)
        };
        let mut footer = div()
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .bg(rgb(0xf7f7f8))
            .child(format!(
                "{}{}",
                title,
                if self.document.is_dirty() || self.has_uncommitted_draft(cx) {
                    "  • 未保存"
                } else {
                    ""
                }
            ))
            .child(self.status.clone());
        for issue in &self.asset_diagnostics {
            let issue = issue.clone();
            footer = footer.child(button(
                cx,
                format!(
                    "画像 · Slide {} · {} · {}",
                    issue.slide_index + 1,
                    issue.path,
                    issue.problem
                ),
                true,
                move |this, _, window, cx| this.navigate_to_asset(issue.clone(), window, cx),
            ));
        }
        for issue in self.layout_diagnostics.clone() {
            let label = format!(
                "Layout · Slide {} · Element {} · {}",
                issue.slide_index + 1,
                issue.element_index + 1,
                issue.message
            );
            footer = footer.child(button(cx, label, true, move |this, _, window, cx| {
                if this.external_edit_blocked() {
                    return;
                }
                let Some(kind) = this
                    .document
                    .model()
                    .and_then(|model| model.slides.get(issue.slide_index))
                    .and_then(|slide| slide.elements.get(issue.element_index))
                    .map(element_kind)
                else {
                    return;
                };
                this.current_slide = issue.slide_index;
                if kind == ElementKind::Columns {
                    this.select_columns(issue.slide_index, issue.element_index, window, cx);
                } else {
                    let field = if kind == ElementKind::Image {
                        ElementField::ImagePath
                    } else {
                        ElementField::Text
                    };
                    this.select_element(
                        issue.slide_index,
                        issue.element_index,
                        field,
                        false,
                        window,
                        cx,
                    );
                }
            }));
        }
        if self.external_edit_blocked() {
            footer = footer.child(
                div()
                    .text_color(rgb(0x991b1b))
                    .child("表示中の内容は現在の外部ファイルと一致していません。"),
            );
        }
        if let ExternalState::Unavailable { message: error, .. } = self.document.external_state() {
            footer = footer.child(
                div()
                    .text_color(rgb(0x991b1b))
                    .child(format!("外部ファイルを読み込めません: {error}")),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .key_context("Editor")
            .track_focus(&self.root_focus)
            .on_mouse_move(cx.listener(|this, event, _, cx| this.update_column_resize(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.finish_column_resize(window, cx);
                    if this.drop_destination.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.finish_column_resize(window, cx);
                    if this.drop_destination.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::commit_text_edit))
            .on_action(cx.listener(Self::copy_input))
            .on_action(cx.listener(Self::cut_input))
            .on_action(cx.listener(Self::paste_input))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::save_as_action))
            .bg(rgb(0xe5e7eb))
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .p_2()
                    .child(side)
                    .child(div().flex_1().justify_center().items_center().child(canvas))
                    .child(inspector),
            )
            .child(footer)
    }

    fn render_slide_list(
        &mut self,
        model: Option<&PresentationModel>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut list = div()
            .flex()
            .flex_col()
            .w(px(150.0))
            .flex_shrink_0()
            .h_full()
            .gap_2()
            .p_2()
            .bg(rgb(0xf3f4f6));
        list = list.child(div().font_weight(gpui::FontWeight::BOLD).child("Slides"));
        if let Some(model) = model {
            for (index, slide) in model.slides.iter().enumerate() {
                let selected = index == self.current_slide;
                let heading = slide
                    .elements
                    .iter()
                    .find_map(|element| match element {
                        Element::Heading(text) => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Slide {}", index + 1));
                let heading = if heading.chars().count() > 7 {
                    format!("{}…", heading.chars().take(6).collect::<String>())
                } else {
                    heading
                };
                let label = format!(
                    "{}{}  {}",
                    if selected { "› " } else { "" },
                    index + 1,
                    heading
                );
                list = list.child(button(cx, label, true, move |this, _, window, cx| {
                    if this.external_edit_blocked() {
                        // Navigate the frozen preview without committing or
                        // replacing the draft belonging to the previous target.
                        this.current_slide = index;
                        window.focus(&this.root_focus, cx);
                        cx.notify();
                        return;
                    }
                    if let Err(error) = this.commit_draft(window, cx) {
                        this.status = error;
                        cx.notify();
                        return;
                    }
                    this.current_slide = index;
                    this.select_title(window, cx);
                }));
            }
        }
        list.into_any_element()
    }

    fn render_element_outline(
        &mut self,
        model: &PresentationModel,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        const PAGE_SIZE: usize = 4;
        let mut outline = div()
            .id("element-outline")
            .flex()
            .flex_col()
            .max_h(px(180.0))
            .flex_shrink_0()
            .gap_1();
        let Some(slide) = model.slides.get(self.current_slide) else {
            return outline.into_any_element();
        };
        let mut rows = Vec::new();
        for (index, element) in slide.elements.iter().enumerate() {
            rows.push((
                CanvasSource::Element {
                    slide: self.current_slide,
                    index,
                },
                format!("Element {} · {}", index + 1, outline_kind_label(element)),
            ));
            if let Element::Columns { left, right, .. } = element {
                for (column, label, elements) in [(0, "左列", left), (1, "右列", right)] {
                    for (nested_index, nested) in elements.iter().enumerate() {
                        rows.push((
                            CanvasSource::NestedElement {
                                slide: self.current_slide,
                                columns: index,
                                column,
                                index: nested_index,
                            },
                            format!(
                                "{label} · {} {}",
                                outline_kind_label(nested),
                                nested_index + 1
                            ),
                        ));
                    }
                }
            }
        }
        let page_count = rows.len().div_ceil(PAGE_SIZE).max(1);
        let page = if self.outline_page.slide == self.current_slide {
            self.outline_page.index.min(page_count - 1)
        } else {
            0
        };
        self.outline_page = OutlinePage {
            slide: self.current_slide,
            index: page,
        };
        for (source, label) in rows.into_iter().skip(page * PAGE_SIZE).take(PAGE_SIZE) {
            let row = self.render_outline_row(source, label, cx);
            outline = outline.child(if matches!(source, CanvasSource::NestedElement { .. }) {
                div().flex_shrink_0().pl_3().child(row).into_any_element()
            } else {
                row
            });
        }
        if page_count > 1 {
            let slide = self.current_slide;
            outline = outline.child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(button(
                        cx,
                        "前の項目",
                        page > 0,
                        move |this, _, _, cx| {
                            this.outline_page = OutlinePage {
                                slide,
                                index: page - 1,
                            };
                            cx.notify();
                        },
                    ))
                    .child(format!("{}/{}", page + 1, page_count))
                    .child(button(
                        cx,
                        "次の項目",
                        page + 1 < page_count,
                        move |this, _, _, cx| {
                            this.outline_page = OutlinePage {
                                slide,
                                index: page + 1,
                            };
                            cx.notify();
                        },
                    )),
            );
        }
        outline.into_any_element()
    }

    fn render_outline_row(
        &self,
        source: CanvasSource,
        label: String,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let selected = match (source, self.target) {
            (
                CanvasSource::Element { slide, index },
                EditTarget::Element {
                    slide: target_slide,
                    index: target_index,
                    ..
                }
                | EditTarget::Columns {
                    slide: target_slide,
                    index: target_index,
                }
                | EditTarget::ColumnWidth {
                    slide: target_slide,
                    index: target_index,
                    ..
                },
            ) => slide == target_slide && index == target_index,
            (
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                },
                EditTarget::NestedElement {
                    slide: target_slide,
                    columns: target_columns,
                    column: target_column,
                    index: target_index,
                    ..
                },
            ) => {
                slide == target_slide
                    && columns == target_columns
                    && column == target_column
                    && index == target_index
            }
            _ => false,
        };
        div()
            .flex_shrink_0()
            .child(button(
                cx,
                format!("{label}{}", if selected { " ✓" } else { "" }),
                true,
                move |this, _, window, cx| this.select_canvas_source(source, window, cx),
            ))
            .into_any_element()
    }

    fn render_inspector(
        &mut self,
        model: Option<&PresentationModel>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut inspector = div()
            .id("inspector")
            .flex()
            .flex_col()
            .w(px(270.0))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .overflow_y_scroll()
            .gap_2()
            .p_3()
            .bg(rgb(0xf8f8f8));
        inspector = inspector.child(div().font_weight(gpui::FontWeight::BOLD).child("Inspector"));
        if self.external_edit_blocked() {
            return inspector
                .child("外部ファイルの問題を解決するまで編集できません。")
                .child("保持中の入力")
                .child(div().w_full().child(self.editor.read(cx).value()))
                .into_any_element();
        }
        if let Some(model) = model {
            inspector = inspector
                .child(div().font_weight(gpui::FontWeight::BOLD).child("Elements"))
                .child(self.render_element_outline(model, cx));
            inspector = inspector.child(button(
                cx,
                "Presentation title",
                true,
                |this, _, window, cx| this.select_title(window, cx),
            ));
            if let EditTarget::NestedElement { slide, columns, .. } = self.target {
                inspector = inspector.child(button(
                    cx,
                    "2列の内容へ戻る",
                    true,
                    move |this, _, window, cx| this.select_columns(slide, columns, window, cx),
                ));
                inspector = inspector.child(button(
                    cx,
                    "Slide 直下へ移動",
                    true,
                    |this, _, window, cx| this.move_nested_element_to_slide(window, cx),
                ));
            }
            let element = match self.target {
                EditTarget::Element { slide, index, .. } => model
                    .slides
                    .get(slide)
                    .and_then(|slide| slide.elements.get(index)),
                EditTarget::Columns { slide, index }
                | EditTarget::ColumnWidth { slide, index, .. } => model
                    .slides
                    .get(slide)
                    .and_then(|slide| slide.elements.get(index)),
                EditTarget::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                    ..
                } => nested_element(model, slide, columns, column, index),
                EditTarget::Title => None,
            };
            if let (EditTarget::Element { slide, .. }, Some(element)) = (self.target, element) {
                if !matches!(element, Element::Columns { .. }) {
                    let destinations = model
                        .slides
                        .get(slide)
                        .into_iter()
                        .flat_map(|slide| slide.elements.iter().enumerate())
                        .filter_map(|(index, element)| {
                            matches!(element, Element::Columns { .. }).then_some(index)
                        })
                        .collect::<Vec<_>>();
                    for columns in destinations {
                        for (column, label) in [(0, "左列"), (1, "右列")] {
                            inspector = inspector.child(button(
                                cx,
                                format!("2列 {} · {label}へ移動", columns + 1),
                                true,
                                move |this, _, window, cx| {
                                    this.move_top_level_element_to_column(
                                        columns, column, window, cx,
                                    )
                                },
                            ));
                        }
                    }
                }
            }
            if let Some(element) = element {
                if let EditTarget::ColumnWidth { left, .. } = self.target {
                    inspector = inspector
                        .child(div().pt_2().child(if left {
                            "左列幅（1〜99）"
                        } else {
                            "右列幅（1〜99）"
                        }))
                        .child(
                            div()
                                .w_full()
                                .px_2()
                                .py_1()
                                .border_1()
                                .border_color(rgb(0xb8bdc5))
                                .bg(rgb(0xffffff))
                                .child(self.editor.clone()),
                        )
                        .child(button(cx, "幅を適用", true, |this, _, window, cx| {
                            match this.commit_draft(window, cx) {
                                Ok(()) => window.focus(&this.root_focus, cx),
                                Err(error) => this.status = error,
                            }
                            cx.notify();
                        }));
                }
                match element {
                    Element::Heading(_) | Element::Text(_) => {}
                    Element::Bullets(items) => {
                        inspector = inspector.child(div().pt_2().child("箇条書き項目"));
                        let item_actions_available = matches!(
                            self.target,
                            EditTarget::Element { .. } | EditTarget::NestedElement { .. }
                        );
                        for (item_index, _) in items.iter().enumerate() {
                            let selected = matches!(
                                self.target,
                                EditTarget::Element {
                                    field: ElementField::Bullet(selected),
                                    ..
                                } | EditTarget::NestedElement {
                                    field: ElementField::Bullet(selected),
                                    ..
                                } if selected == item_index
                            );
                            inspector = inspector.child(button(
                                cx,
                                format!(
                                    "項目 {}{}",
                                    item_index + 1,
                                    if selected { " ✓" } else { "" }
                                ),
                                true,
                                move |this, _, window, cx| {
                                    this.select_current_field(
                                        ElementField::Bullet(item_index),
                                        window,
                                        cx,
                                    )
                                },
                            ));
                            if selected && item_actions_available {
                                inspector = inspector
                                    .child(button(
                                        cx,
                                        "項目を上へ",
                                        item_index > 0,
                                        |this, _, window, cx| {
                                            this.move_bullet_selection(-1, window, cx)
                                        },
                                    ))
                                    .child(button(
                                        cx,
                                        "項目を下へ",
                                        item_index + 1 < items.len(),
                                        |this, _, window, cx| {
                                            this.move_bullet_selection(1, window, cx)
                                        },
                                    ))
                                    .child(button(
                                        cx,
                                        "項目を削除",
                                        true,
                                        |this, _, window, cx| {
                                            this.remove_bullet_selection(window, cx)
                                        },
                                    ));
                            }
                        }
                        if item_actions_available {
                            inspector = inspector.child(button(
                                cx,
                                "項目を追加",
                                true,
                                |this, _, window, cx| this.add_bullet_selection(window, cx),
                            ));
                        }
                    }
                    Element::Code { .. } => {
                        inspector = inspector.child(button(
                            cx,
                            "コード言語",
                            true,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::CodeLanguage, window, cx)
                            },
                        ));
                    }
                    Element::Image { .. } => {
                        inspector = inspector.child(button(
                            cx,
                            "画像パス",
                            true,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::ImagePath, window, cx)
                            },
                        ));
                        inspector =
                            inspector.child(button(cx, "Caption", true, |this, _, window, cx| {
                                this.select_current_field(ElementField::Caption, window, cx)
                            }));
                        inspector = inspector.child(button(
                            cx,
                            "画像を再読み込み",
                            true,
                            |this, _, _, cx| {
                                this.refresh_assets();
                                this.status = if this.asset_diagnostics.is_empty() {
                                    "画像を再読み込みしました。".into()
                                } else {
                                    "画像の診断を更新しました。".into()
                                };
                                cx.notify();
                            },
                        ));
                    }
                    Element::Columns {
                        left_width,
                        right_width,
                        ..
                    } => {
                        inspector =
                            inspector.child(format!("左列 {left_width}% / 右列 {right_width}%"));
                        inspector = inspector.child(button(
                            cx,
                            "Element を削除",
                            true,
                            Self::delete_element,
                        ));
                        inspector = inspector.child(button(
                            cx,
                            "左列幅を数値入力",
                            true,
                            |this, _, window, cx| {
                                let (slide, index) = match this.target {
                                    EditTarget::Columns { slide, index }
                                    | EditTarget::ColumnWidth { slide, index, .. } => {
                                        (slide, index)
                                    }
                                    _ => return,
                                };
                                this.select_column_width(slide, index, true, window, cx);
                            },
                        ));
                        inspector = inspector.child(button(
                            cx,
                            "右列幅を数値入力",
                            true,
                            |this, _, window, cx| {
                                let (slide, index) = match this.target {
                                    EditTarget::Columns { slide, index }
                                    | EditTarget::ColumnWidth { slide, index, .. } => {
                                        (slide, index)
                                    }
                                    _ => return,
                                };
                                this.select_column_width(slide, index, false, window, cx);
                            },
                        ));
                        inspector = inspector.child(button(
                            cx,
                            "左列を +5%",
                            *left_width <= 94,
                            |this, _, _, cx| this.resize_columns(5, cx),
                        ));
                        inspector = inspector.child(button(
                            cx,
                            "左列を -5%",
                            *left_width >= 6,
                            |this, _, _, cx| this.resize_columns(-5, cx),
                        ));
                        for (column, label) in [(0, "左列"), (1, "右列")] {
                            inspector = inspector.child(
                                div()
                                    .pt_2()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(label),
                            );
                            if self.column_add_menu == Some(column) {
                                for (kind, kind_label) in [
                                    (ElementKind::Heading, "見出し"),
                                    (ElementKind::Text, "本文"),
                                    (ElementKind::Bullets, "箇条書き"),
                                    (ElementKind::Code, "コード"),
                                    (ElementKind::Image, "画像"),
                                ] {
                                    inspector = inspector.child(button(
                                        cx,
                                        format!("{label}に{kind_label}を追加"),
                                        true,
                                        move |this, _, window, cx| {
                                            this.add_column_element_selection(
                                                column, kind, window, cx,
                                            )
                                        },
                                    ));
                                }
                            } else {
                                inspector = inspector.child(button(
                                    cx,
                                    format!("{label}に追加…"),
                                    true,
                                    move |this, _, _, cx| {
                                        this.column_add_menu = Some(column);
                                        cx.notify();
                                    },
                                ));
                            }
                        }
                    }
                }
                if self.canvas_editing && supports_canvas_text_editor(element, self.target) {
                    inspector = inspector
                        .child(
                            div()
                                .pt_2()
                                .text_size(px(12.0))
                                .child("キャンバス上で編集中"),
                        )
                        .child(button(
                            cx,
                            "全文をInspectorで編集",
                            true,
                            |this, _, window, cx| {
                                // Keep the same draft and IME session, but mount the input once.
                                this.canvas_editing = false;
                                this.ime_geometry.refresh(window, true);
                                let focus = this.editor.read(cx).focus_handle();
                                window.focus(&focus, cx);
                                cx.notify();
                            },
                        ));
                } else if !matches!(element, Element::Columns { .. }) {
                    inspector = inspector
                        .child(div().pt_2().child("編集値"))
                        .child(if self.editor.read(cx).is_multiline() {
                            div().text_size(px(11.0)).child("Enter で改行できます")
                        } else {
                            div().text_size(px(11.0)).child("1 行で編集します")
                        })
                        .child(
                            div()
                                .w_full()
                                .px_2()
                                .py_1()
                                .border_1()
                                .border_color(rgb(0xb8bdc5))
                                .bg(rgb(0xffffff))
                                .child(self.editor.clone()),
                        )
                        .child(button(cx, "適用", true, |this, _, window, cx| {
                            match this.commit_draft(window, cx) {
                                Ok(()) => window.focus(&this.root_focus, cx),
                                Err(error) => this.status = error,
                            }
                            cx.notify();
                        }));
                }
                let selected_element = match self.target {
                    EditTarget::Element { slide, index, .. }
                    | EditTarget::Columns { slide, index }
                    | EditTarget::ColumnWidth { slide, index, .. } => Some((slide, index)),
                    EditTarget::NestedElement { .. } | EditTarget::Title => None,
                };
                if let Some((slide, index)) = selected_element {
                    let count = model
                        .slides
                        .get(slide)
                        .map_or(0, |slide| slide.elements.len());
                    if count > 1 {
                        inspector = inspector
                            .child(button(
                                cx,
                                "Element ↑",
                                index > 0,
                                |this, _, window, cx| this.move_element_selection(-1, window, cx),
                            ))
                            .child(button(
                                cx,
                                "Element ↓",
                                index + 1 < count,
                                |this, _, window, cx| this.move_element_selection(1, window, cx),
                            ));
                    }
                }
                if let EditTarget::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                    ..
                } = self.target
                {
                    let count = model
                        .slides
                        .get(slide)
                        .and_then(|slide| slide.elements.get(columns))
                        .and_then(|element| match element {
                            Element::Columns { left, right, .. } => {
                                Some(if column == 0 { left } else { right })
                            }
                            _ => None,
                        })
                        .map_or(0, Vec::len);
                    if count > 1 {
                        inspector = inspector
                            .child(button(
                                cx,
                                "列内 Element ↑",
                                index > 0,
                                |this, _, window, cx| {
                                    this.move_nested_element_selection(-1, window, cx)
                                },
                            ))
                            .child(button(
                                cx,
                                "列内 Element ↓",
                                index + 1 < count,
                                |this, _, window, cx| {
                                    this.move_nested_element_selection(1, window, cx)
                                },
                            ));
                    }
                }
                if !matches!(element, Element::Columns { .. }) {
                    inspector =
                        inspector.child(button(cx, "Element を削除", true, Self::delete_element));
                }
            } else if let EditTarget::Title = self.target {
                inspector = inspector
                    .child(div().pt_2().child("編集値"))
                    .child(
                        div()
                            .h(px(38.0))
                            .w_full()
                            .px_2()
                            .py_1()
                            .border_1()
                            .border_color(rgb(0xb8bdc5))
                            .bg(rgb(0xffffff))
                            .child(self.editor.clone()),
                    )
                    .child(button(cx, "適用", true, |this, _, window, cx| {
                        if let Err(error) = this.commit_draft(window, cx) {
                            this.status = error;
                        } else {
                            window.focus(&this.root_focus, cx);
                        }
                        cx.notify();
                    }));
            }
            if !matches!(self.target, EditTarget::Columns { .. }) {
                if let Err(DocumentError::UnsafeValue(reason)) =
                    validate_editor_value(self.target, &self.editor.read(cx).value())
                {
                    inspector = inspector.child(div().text_color(rgb(0x991b1b)).child(reason));
                }
                inspector = inspector.child(button(
                    cx,
                    "入力を取り消す",
                    self.has_uncommitted_draft(cx),
                    |this, _, window, cx| this.cancel_input(window, cx),
                ));
                inspector = inspector
                    .child(button(
                        cx,
                        "見出しを追加",
                        true,
                        |this, _, window, cx| this.add_element(ElementKind::Heading, window, cx),
                    ))
                    .child(button(
                        cx,
                        "本文を追加",
                        true,
                        |this, _, window, cx| this.add_element(ElementKind::Text, window, cx),
                    ))
                    .child(button(
                        cx,
                        "箇条書きを追加",
                        true,
                        |this, _, window, cx| this.add_element(ElementKind::Bullets, window, cx),
                    ))
                    .child(button(
                        cx,
                        "コードを追加",
                        true,
                        |this, _, window, cx| this.add_element(ElementKind::Code, window, cx),
                    ))
                    .child(button(
                        cx,
                        "画像参照を追加",
                        true,
                        |this, _, window, cx| this.add_element(ElementKind::Image, window, cx),
                    ))
                    .child(button(cx, "2列を追加", true, |this, _, window, cx| {
                        this.add_element(ElementKind::Columns, window, cx)
                    }));
            }
        }
        inspector.into_any_element()
    }

    fn select_current_field(
        &mut self,
        field: ElementField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.target {
            EditTarget::Element { slide, index, .. } => {
                self.select_element(slide, index, field, true, window, cx)
            }
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => self.select_nested_element(slide, columns, column, index, field, true, window, cx),
            EditTarget::Columns { .. } | EditTarget::ColumnWidth { .. } | EditTarget::Title => {}
        }
    }

    fn resize_columns(&mut self, delta: i16, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        let (slide, index) = match self.target {
            EditTarget::Columns { slide, index } | EditTarget::ColumnWidth { slide, index, .. } => {
                (slide, index)
            }
            _ => return,
        };
        let Some(Element::Columns { left_width, .. }) = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
            .and_then(|slide| slide.elements.get(index))
        else {
            return;
        };
        let left = (*left_width as i16 + delta).clamp(1, 99) as u8;
        if let Err(error) = self
            .document
            .set_column_widths(slide, index, left, 100 - left)
        {
            self.status = format!("列幅を変更できません: {error}");
        } else {
            self.status = "列幅を変更しました。".into();
        }
        cx.notify();
    }

    fn begin_column_resize(
        &mut self,
        slide: usize,
        index: usize,
        start_x: f32,
        start_width: u8,
        usable_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.external_edit_blocked() {
            return;
        }
        if self.has_uncommitted_draft(cx) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = format!("列幅を変更できません: {error}");
                cx.notify();
                return;
            }
        }
        let start_width = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
            .and_then(|slide| slide.elements.get(index))
            .and_then(|element| match element {
                Element::Columns { left_width, .. } => Some(*left_width),
                _ => None,
            })
            .unwrap_or(start_width);
        self.current_slide = slide;
        self.column_resize_drag = Some(ColumnResizeDrag {
            slide,
            index,
            start_x,
            start_width,
            preview_width: start_width,
            usable_width: usable_width.max(1.0),
        });
        self.select_columns(slide, index, window, cx);
        cx.notify();
    }

    fn update_column_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(mut drag) = self.column_resize_drag else {
            return;
        };
        if !event.dragging() || self.external_edit_blocked() {
            return;
        }
        let delta = f32::from(event.position.x) - drag.start_x;
        let width_delta = delta / drag.usable_width * 100.0;
        let preview = (f32::from(drag.start_width) + width_delta)
            .round()
            .clamp(1.0, 99.0) as u8;
        if drag.preview_width != preview {
            drag.preview_width = preview;
            self.column_resize_drag = Some(drag);
            cx.notify();
        }
    }

    fn finish_column_resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.column_resize_drag.take() else {
            return;
        };
        if self.external_edit_blocked() {
            self.status = "外部ファイルの競合を解決するまで列幅を変更できません。".into();
            cx.notify();
            return;
        }
        if drag.preview_width != drag.start_width {
            match self.document.set_column_widths(
                drag.slide,
                drag.index,
                drag.preview_width,
                100 - drag.preview_width,
            ) {
                Ok(()) => self.status = "列幅を変更しました。".into(),
                Err(error) => self.status = format!("列幅を変更できません: {error}"),
            }
        }
        self.select_columns(drag.slide, drag.index, window, cx);
        cx.notify();
    }

    fn update_drop_destination(
        &mut self,
        destination: DropDestination,
        event: &DragMoveEvent<DraggedCanvasElement>,
        cx: &mut Context<Self>,
    ) {
        let destination = match destination {
            DropDestination::Element {
                slide,
                index,
                after: _,
            } => DropDestination::Element {
                slide,
                index,
                after: event.event.position.y > event.bounds.center().y,
            },
            DropDestination::NestedElement {
                slide,
                columns,
                column,
                index,
                after: _,
            } => DropDestination::NestedElement {
                slide,
                columns,
                column,
                index,
                after: event.event.position.y > event.bounds.center().y,
            },
            column @ DropDestination::Column { .. } => column,
        };
        if self.drop_destination != Some(destination) {
            self.drop_destination = Some(destination);
            cx.notify();
        }
    }

    fn apply_dragged_element(
        &mut self,
        dragged: DraggedCanvasElement,
        destination: DropDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_destination = None;
        if self.external_edit_blocked() {
            return;
        }
        if self.has_uncommitted_draft(cx) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = format!("Element を移動できません: {error}");
                cx.notify();
                return;
            }
        }

        let source = dragged.source;
        let result = match (source, destination) {
            (
                CanvasSource::Element { slide, index: from },
                DropDestination::Element {
                    slide: target_slide,
                    index: target,
                    after,
                },
            ) if slide == target_slide => {
                let count = self
                    .document
                    .model()
                    .and_then(|model| model.slides.get(slide))
                    .map_or(0, |slide| slide.elements.len());
                let boundary = target + usize::from(after);
                let to = insertion_after_removal(from, boundary, count);
                self.document
                    .move_element(slide, from, to)
                    .map(|()| CanvasSource::Element { slide, index: to })
            }
            (
                CanvasSource::Element { slide, index },
                DropDestination::NestedElement {
                    slide: target_slide,
                    columns,
                    column,
                    index: target,
                    after,
                },
            ) if slide == target_slide => {
                let count = nested_column_len(&self.document, slide, columns, column);
                let destination_index = (target + usize::from(after)).min(count);
                self.document
                    .move_element_to_column_at(slide, index, columns, column, destination_index)
                    .map(|(columns, index)| CanvasSource::NestedElement {
                        slide,
                        columns,
                        column,
                        index,
                    })
            }
            (
                CanvasSource::Element { slide, index },
                DropDestination::Column {
                    slide: target_slide,
                    columns,
                    column,
                },
            ) if slide == target_slide => self
                .document
                .move_element_to_column(slide, index, columns, column)
                .map(|(columns, index)| CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                }),
            (
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                },
                DropDestination::Element {
                    slide: target_slide,
                    index: target,
                    after,
                },
            ) if slide == target_slide => {
                let count = self
                    .document
                    .model()
                    .and_then(|model| model.slides.get(slide))
                    .map_or(0, |slide| slide.elements.len());
                let destination_index = (target + usize::from(after)).min(count);
                self.document
                    .move_column_element_to_slide_at(
                        slide,
                        columns,
                        column,
                        index,
                        destination_index,
                    )
                    .map(|index| CanvasSource::Element { slide, index })
            }
            (
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column: source_column,
                    index: from,
                },
                DropDestination::NestedElement {
                    slide: target_slide,
                    columns: target_columns,
                    column,
                    index: target,
                    after,
                },
            ) if slide == target_slide => {
                if source_column == column {
                    if columns == target_columns {
                        let count = nested_column_len(&self.document, slide, columns, column);
                        let boundary = target + usize::from(after);
                        let to = insertion_after_removal(from, boundary, count);
                        self.document
                            .move_column_element(slide, columns, column, from, to)
                            .map(|()| CanvasSource::NestedElement {
                                slide,
                                columns,
                                column,
                                index: to,
                            })
                    } else {
                        let count =
                            nested_column_len(&self.document, slide, target_columns, column);
                        let destination_index = (target + usize::from(after)).min(count);
                        self.document
                            .move_column_element_between_columns_at(
                                slide,
                                columns,
                                source_column,
                                from,
                                target_columns,
                                column,
                                destination_index,
                            )
                            .map(|index| CanvasSource::NestedElement {
                                slide,
                                columns: target_columns,
                                column,
                                index,
                            })
                    }
                } else {
                    let count = nested_column_len(&self.document, slide, target_columns, column);
                    let destination_index = (target + usize::from(after)).min(count);
                    self.document
                        .move_column_element_between_columns_at(
                            slide,
                            columns,
                            source_column,
                            from,
                            target_columns,
                            column,
                            destination_index,
                        )
                        .map(|index| CanvasSource::NestedElement {
                            slide,
                            columns: target_columns,
                            column,
                            index,
                        })
                }
            }
            (
                CanvasSource::NestedElement {
                    slide,
                    columns,
                    column: source_column,
                    index,
                },
                DropDestination::Column {
                    slide: target_slide,
                    columns: target_columns,
                    column,
                },
            ) if slide == target_slide => {
                let destination_index =
                    nested_column_len(&self.document, slide, target_columns, column);
                self.document
                    .move_column_element_between_columns_at(
                        slide,
                        columns,
                        source_column,
                        index,
                        target_columns,
                        column,
                        destination_index,
                    )
                    .map(|index| CanvasSource::NestedElement {
                        slide,
                        columns: target_columns,
                        column,
                        index,
                    })
            }
            _ => return,
        };
        match result {
            Ok(target) => {
                // Reordering can invalidate the previous numeric target.
                // Clear it before using the ordinary selection helpers,
                // which commit the previous field when changing selection.
                self.reset_editor_to_title(cx);
                self.select_canvas_source(target, window, cx);
                self.refresh_assets();
                self.status = "Element をドラッグして移動しました。".into();
            }
            Err(error) => self.status = format!("Element を移動できません: {error}"),
        }
        cx.notify();
    }

    fn select_canvas_source(
        &mut self,
        source: CanvasSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match source {
            CanvasSource::Element { slide, index } => {
                let Some(element) = self
                    .document
                    .model()
                    .and_then(|model| model.slides.get(slide))
                    .and_then(|slide| slide.elements.get(index))
                else {
                    return;
                };
                if matches!(element, Element::Columns { .. }) {
                    self.select_columns(slide, index, window, cx);
                } else {
                    self.select_element(
                        slide,
                        index,
                        default_edit_field(element),
                        false,
                        window,
                        cx,
                    );
                }
            }
            CanvasSource::NestedElement {
                slide,
                columns,
                column,
                index,
            } => {
                let Some(element) = self
                    .document
                    .model()
                    .and_then(|model| nested_element(model, slide, columns, column, index))
                else {
                    return;
                };
                self.select_nested_element(
                    slide,
                    columns,
                    column,
                    index,
                    default_edit_field(element),
                    false,
                    window,
                    cx,
                );
            }
        }
    }

    fn render_canvas(
        &mut self,
        model: &PresentationModel,
        slide_index: usize,
        editing: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let viewport = window.viewport_size();
        // 150px slide list + 270px inspector + 16px outer padding + 16px gaps.
        let horizontal_panes = if editing { 452.0 } else { 0.0 };
        let available_width = (f32::from(viewport.width) - horizontal_panes).max(320.0);
        let available_height =
            (f32::from(viewport.height) - if editing { 72.0 } else { 0.0 }).max(180.0);
        let scale = (available_width / 1280.0).min(available_height / 720.0);
        let width = 1280.0 * scale;
        let height = 720.0 * scale;
        let slide = model.slides.get(slide_index);
        let canvas_origin = std::rc::Rc::new(std::cell::Cell::new(gpui::point(px(0.), px(0.))));
        let placement = text::TextPlacement::new(canvas_origin.clone(), 64., 48.);
        let mut cursor_y = 0.;
        let mut canvas = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(24.0 * scale))
            .w(px(width))
            .h(px(height))
            .px(px(64.0 * scale))
            .py(px(48.0 * scale))
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222));
        canvas = canvas.child(
            gpui::canvas(
                move |bounds, _, _| {
                    canvas_origin.set(bounds.origin);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );
        if let Some(slide) = slide {
            let images = if editing {
                &self.asset_images
            } else {
                self.presentation_assets
                    .as_ref()
                    .unwrap_or(&self.asset_images)
            };
            for (index, element) in slide.elements.iter().enumerate() {
                let selected = editing
                    && (matches!(self.target, EditTarget::Element { slide: selected_slide, index: selected_index, .. } if selected_slide == slide_index && selected_index == index)
                        || matches!(self.target, EditTarget::Columns { slide: selected_slide, index: selected_index } if selected_slide == slide_index && selected_index == index)
                        || matches!(self.target, EditTarget::ColumnWidth { slide: selected_slide, index: selected_index, .. } if selected_slide == slide_index && selected_index == index)
                        || matches!(self.target, EditTarget::NestedElement { slide: selected_slide, columns, .. } if selected_slide == slide_index && columns == index));
                // Place complete Elements using the same base coordinates
                // as their glyphs. Flex's per-child rounding otherwise
                // accumulates before images and code backgrounds.
                canvas = canvas.child(
                    div()
                        .absolute()
                        .left(px(64. * scale))
                        .top(px((48. + cursor_y) * scale))
                        .w(px(width - 128. * scale))
                        .child(self.render_canvas_element(
                            element,
                            scale,
                            selected,
                            images,
                            width - 128.0 * scale,
                            CanvasTarget::Element {
                                slide: slide_index,
                                index,
                            },
                            editing && !self.external_edit_blocked(),
                            window,
                            &placement.shifted(0., cursor_y),
                            cx,
                        )),
                );
                cursor_y += measure_element(element, 1152., window, &mut Vec::new()) + 24.;
            }
        }
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(if editing {
                rgb(0xe5e7eb)
            } else {
                rgb(0x000000)
            })
            .child(canvas)
            .into_any_element()
    }

    fn render_canvas_element(
        &self,
        element: &Element,
        scale: f32,
        selected: bool,
        images: &HashMap<String, Arc<gpui::RenderImage>>,
        content_width: f32,
        target: CanvasTarget,
        editing: bool,
        window: &Window,
        placement: &text::TextPlacement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let field = match element {
            Element::Image { .. } => ElementField::ImagePath,
            Element::Columns { .. } => ElementField::Text,
            _ => ElementField::Text,
        };
        let inline_editing = editing
            && self.canvas_editing
            && selected
            && self.editor.read(cx).has_focus(window)
            && match (target, self.target) {
                (
                    CanvasTarget::Element { slide, index },
                    EditTarget::Element {
                        slide: selected_slide,
                        index: selected_index,
                        field: ElementField::Text,
                    },
                ) => slide == selected_slide && index == selected_index,
                (
                    CanvasTarget::NestedElement {
                        slide,
                        columns,
                        column,
                        index,
                    },
                    EditTarget::NestedElement {
                        slide: selected_slide,
                        columns: selected_columns,
                        column: selected_column,
                        index: selected_index,
                        field: ElementField::Text,
                    },
                ) => {
                    slide == selected_slide
                        && columns == selected_columns
                        && column == selected_column
                        && index == selected_index
                }
                _ => false,
            }
            && matches!(
                element,
                Element::Heading(_) | Element::Text(_) | Element::Bullets(_) | Element::Code { .. }
            );
        let rendered = match element {
            Element::Image { path, caption } => {
                let image = if let Some(image) = images.get(path) {
                    div()
                        .h(px(320.0 * scale))
                        .w_full()
                        .flex()
                        .justify_center()
                        .items_center()
                        .bg(rgb(0xffffff))
                        .child(
                            gpui::img(image.clone())
                                .object_fit(gpui::ObjectFit::Contain)
                                .w_full()
                                .h_full(),
                        )
                } else {
                    div()
                        .h(px(320.0 * scale))
                        .w_full()
                        .flex()
                        .justify_center()
                        .items_center()
                        .bg(rgb(0xe5e7eb))
                        .text_size(px(18.0 * scale))
                        .child(format!("画像を読み込めません: {path}"))
                };
                let caption_selected = matches!(
                    (target, self.target),
                    (
                        CanvasTarget::Element { slide, index },
                        EditTarget::Element {
                            slide: selected_slide,
                            index: selected_index,
                            field: ElementField::Caption,
                        }
                    ) if slide == selected_slide && index == selected_index
                ) || matches!(
                    (target, self.target),
                    (
                        CanvasTarget::NestedElement {
                            slide,
                            columns,
                            column,
                            index,
                        },
                        EditTarget::NestedElement {
                            slide: selected_slide,
                            columns: selected_columns,
                            column: selected_column,
                            index: selected_index,
                            field: ElementField::Caption,
                        }
                    ) if slide == selected_slide
                        && columns == selected_columns
                        && column == selected_column
                        && index == selected_index
                );
                let caption_editor_focused = editing
                    && self.canvas_editing
                    && caption_selected
                    && self.editor.read(cx).has_focus(window);
                let mut item = div()
                    .relative()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(12.0 * scale))
                    .when(selected, |item| item.border_1().border_color(rgb(0x3b82f6)))
                    .text_color(rgb(0x222222))
                    .child(image);
                if caption_editor_focused {
                    self.editor.update(cx, |input, _| {
                        input.set_canvas_layout(Some(input::CanvasInputLayout {
                            font_size: 20.,
                            line_height: 28.,
                            width: content_width / scale,
                            scale,
                            placement: placement.shifted(0., 332.),
                            code: false,
                            language: None,
                            bullets: false,
                        }))
                    });
                    item = item.child(
                        div()
                            .w_full()
                            .border_1()
                            .border_color(rgb(0x3b82f6))
                            .text_size(px(20.0 * scale))
                            .line_height(px(28.0 * scale))
                            .child(self.editor.clone()),
                    );
                } else if let Some(caption) = caption.as_ref().filter(|caption| !caption.is_empty())
                {
                    let mut caption_view = div()
                        .w_full()
                        .text_size(px(20.0 * scale))
                        .line_height(px(28.0 * scale))
                        .child(text::shared_text(
                            caption,
                            ".SystemUIFont",
                            20.0,
                            28.0,
                            content_width / scale,
                            false,
                            Vec::new(),
                            scale,
                            window,
                            &placement.shifted(0., 332.),
                        ));
                    if editing {
                        caption_view = caption_view.cursor_pointer().on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, event, window, cx| {
                                if cx.has_active_drag() {
                                    return;
                                }
                                select_canvas_target(
                                    this,
                                    target,
                                    ElementField::Caption,
                                    event,
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            }),
                        );
                    }
                    item = item.child(caption_view);
                } else if editing {
                    item = item.child(
                        div()
                            .absolute()
                            .top(px(324.0 * scale))
                            .w_full()
                            .h(px(20.0 * scale))
                            .px_2()
                            .border_1()
                            .border_color(rgb(0xd1d5db))
                            .text_size(px(14.0 * scale))
                            .line_height(px(20.0 * scale))
                            .text_color(rgb(0x6b7280))
                            .child("＋ キャプションを追加")
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, event, window, cx| {
                                    if cx.has_active_drag() {
                                        return;
                                    }
                                    select_canvas_target(
                                        this,
                                        target,
                                        ElementField::Caption,
                                        event,
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            ),
                    );
                }
                item
            }
            Element::Columns {
                left_width,
                left,
                right,
                ..
            } => {
                let column_content_width = (content_width - 24.0 * scale).max(1.0);
                let (parent_slide, parent_columns) = match target {
                    CanvasTarget::Element { slide, index } => (slide, index),
                    CanvasTarget::NestedElement { .. } => unreachable!(),
                };
                let preview_width = self
                    .column_resize_drag
                    .filter(|drag| drag.slide == parent_slide && drag.index == parent_columns)
                    .map_or(*left_width, |drag| drag.preview_width);
                let left_column_width = column_content_width * f32::from(preview_width) / 100.0;
                let right_column_width =
                    column_content_width * f32::from(100 - preview_width) / 100.0;
                let mut left_column = div()
                    .flex()
                    .flex_col()
                    .gap(px(24.0 * scale))
                    .w(px(left_column_width));
                let mut cursor_y = 0.;
                for (index, nested) in left.iter().enumerate() {
                    let nested_selected = matches!(
                        self.target,
                        EditTarget::NestedElement {
                            slide: selected_slide,
                            columns,
                            column: 0,
                            index: selected_index,
                            ..
                        } if selected_slide == parent_slide
                            && columns == parent_columns
                            && selected_index == index
                    );
                    left_column = left_column.child(self.render_canvas_element(
                        nested,
                        scale,
                        nested_selected,
                        images,
                        left_column_width,
                        CanvasTarget::NestedElement {
                            slide: parent_slide,
                            columns: parent_columns,
                            column: 0,
                            index,
                        },
                        editing,
                        window,
                        &placement.shifted(0., cursor_y),
                        cx,
                    ));
                    cursor_y +=
                        measure_element(nested, left_column_width / scale, window, &mut Vec::new())
                            + 24.;
                }
                let left_destination = DropDestination::Column {
                    slide: parent_slide,
                    columns: parent_columns,
                    column: 0,
                };
                let left_column = if editing {
                    left_column
                        .id(format!("drop-column-{parent_slide}-{parent_columns}-0"))
                        .on_drag_move(cx.listener(
                            move |this, event: &DragMoveEvent<DraggedCanvasElement>, _, cx| {
                                this.update_drop_destination(left_destination, event, cx);
                            },
                        ))
                        .drag_over::<DraggedCanvasElement>(|style, _, _, _| {
                            style
                                .bg(rgb(0xdbeafe))
                                .border_1()
                                .border_color(rgb(0x2563eb))
                        })
                        .on_drop(cx.listener(
                            move |this, dragged: &DraggedCanvasElement, window, cx| {
                                this.apply_dragged_element(*dragged, left_destination, window, cx);
                                cx.stop_propagation();
                            },
                        ))
                        .into_any_element()
                } else {
                    left_column.into_any_element()
                };
                let mut right_column = div()
                    .flex()
                    .flex_col()
                    .gap(px(24.0 * scale))
                    .w(px(right_column_width));
                let mut cursor_y = 0.;
                for (index, nested) in right.iter().enumerate() {
                    let nested_selected = matches!(
                        self.target,
                        EditTarget::NestedElement {
                            slide: selected_slide,
                            columns,
                            column: 1,
                            index: selected_index,
                            ..
                        } if selected_slide == parent_slide
                            && columns == parent_columns
                            && selected_index == index
                    );
                    right_column = right_column.child(self.render_canvas_element(
                        nested,
                        scale,
                        nested_selected,
                        images,
                        right_column_width,
                        CanvasTarget::NestedElement {
                            slide: parent_slide,
                            columns: parent_columns,
                            column: 1,
                            index,
                        },
                        editing,
                        window,
                        &placement.shifted(left_column_width / scale + 24., cursor_y),
                        cx,
                    ));
                    cursor_y += measure_element(
                        nested,
                        right_column_width / scale,
                        window,
                        &mut Vec::new(),
                    ) + 24.;
                }
                let right_destination = DropDestination::Column {
                    slide: parent_slide,
                    columns: parent_columns,
                    column: 1,
                };
                let right_column = if editing {
                    right_column
                        .id(format!("drop-column-{parent_slide}-{parent_columns}-1"))
                        .on_drag_move(cx.listener(
                            move |this, event: &DragMoveEvent<DraggedCanvasElement>, _, cx| {
                                this.update_drop_destination(right_destination, event, cx);
                            },
                        ))
                        .drag_over::<DraggedCanvasElement>(|style, _, _, _| {
                            style
                                .bg(rgb(0xdbeafe))
                                .border_1()
                                .border_color(rgb(0x2563eb))
                        })
                        .on_drop(cx.listener(
                            move |this, dragged: &DraggedCanvasElement, window, cx| {
                                this.apply_dragged_element(*dragged, right_destination, window, cx);
                                cx.stop_propagation();
                            },
                        ))
                        .into_any_element()
                } else {
                    right_column.into_any_element()
                };
                let divider = if editing {
                    div().w(px(24.0 * scale)).flex().justify_center().child(
                        div()
                            .w(px(4.0 * scale))
                            .h_full()
                            .bg(rgb(0xd1d5db))
                            .cursor_col_resize()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    this.begin_column_resize(
                                        parent_slide,
                                        parent_columns,
                                        f32::from(event.position.x),
                                        preview_width,
                                        column_content_width,
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                } else {
                    div().w(px(24.0 * scale))
                };
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .when(selected, |item| item.border_1().border_color(rgb(0x3b82f6)))
                    .child(left_column)
                    .child(divider)
                    .child(right_column)
            }
            _ if inline_editing => render_inline_editor(
                element,
                scale,
                content_width / scale,
                placement,
                self.editor.clone(),
                cx,
            ),
            Element::Bullets(items) if editing && items.is_empty() => div()
                .w_full()
                .h(px(42.0 * scale))
                .text_size(px(28.0 * scale))
                .line_height(px(42.0 * scale))
                .text_color(rgb(0x9ca3af))
                .overflow_hidden()
                .whitespace_nowrap()
                .when(selected, |item| item.border_1().border_color(rgb(0x3b82f6)))
                .child("空の箇条書き"),
            _ => render_element(
                element,
                scale,
                selected,
                images,
                content_width,
                window,
                placement,
            ),
        };
        let rendered = rendered.flex_shrink_0();
        if !editing {
            return rendered.into_any_element();
        }
        let selected_view = rendered
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    if this.editor.read(cx).has_focus(window)
                        && (canvas_target_matches_edit_target(target, field, this.target)
                            || canvas_target_matches_edit_target(
                                target,
                                ElementField::Caption,
                                this.target,
                            ))
                    {
                        window.prevent_default();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    if cx.has_active_drag() || this.column_resize_drag.is_some() {
                        return;
                    }
                    select_canvas_target(this, target, field, event, window, cx);
                    cx.stop_propagation();
                }),
            );
        let destination = drop_destination(target);
        let insert_before = self.drop_destination == Some(destination);
        let after_destination = match destination {
            DropDestination::Element { slide, index, .. } => DropDestination::Element {
                slide,
                index,
                after: true,
            },
            DropDestination::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => DropDestination::NestedElement {
                slide,
                columns,
                column,
                index,
                after: true,
            },
            DropDestination::Column { .. } => destination,
        };
        let insert_after = self.drop_destination == Some(after_destination);
        let default_destination = destination;
        div()
            .id(canvas_target_id(target))
            .w_full()
            .flex_shrink_0()
            .child(selected_view)
            .on_drag(
                DraggedCanvasElement {
                    source: canvas_source(target),
                },
                |_, _, _, cx| cx.new(|_| DragGhost),
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DraggedCanvasElement>, _, cx| {
                    this.update_drop_destination(default_destination, event, cx);
                },
            ))
            .drag_over::<DraggedCanvasElement>(move |style, _, _, _| {
                if insert_before {
                    style.border_t_2().border_color(rgb(0x2563eb))
                } else if insert_after {
                    style.border_b_2().border_color(rgb(0x2563eb))
                } else {
                    style
                }
            })
            .on_drop(
                cx.listener(move |this, dragged: &DraggedCanvasElement, window, cx| {
                    let destination = this.drop_destination.unwrap_or(default_destination);
                    this.apply_dragged_element(*dragged, destination, window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|_, _, window, cx| {
                    cx.defer_in(window, |this, _, cx| {
                        if this.drop_destination.take().is_some() {
                            cx.notify();
                        }
                    });
                }),
            )
            .into_any_element()
    }
}

impl PsychoApp {
    fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.read(cx).has_focus(window) {
            return;
        }
        let target = self.target;
        if !matches!(target, EditTarget::Title) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = error;
                cx.notify();
                return;
            }
            let result = match target {
                EditTarget::Element { slide, index, .. }
                | EditTarget::Columns { slide, index }
                | EditTarget::ColumnWidth { slide, index, .. } => {
                    self.document.remove_element(slide, index)
                }
                EditTarget::NestedElement {
                    slide,
                    columns,
                    column,
                    index,
                    ..
                } => self
                    .document
                    .remove_column_element(slide, columns, column, index),
                EditTarget::Title => unreachable!(),
            };
            if let Err(error) = result {
                self.status = error.to_string();
            } else {
                self.reset_editor_to_title(cx);
                self.status = "Element を削除しました。".into();
            }
        }
        cx.notify();
    }
}

impl Render for PsychoApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep opt-in acceptance observations current when an unfocused
        // canvas field is no longer mounted; paint updates its geometry later.
        self.editor.read(cx).record_acceptance_state(window);
        let editing_enabled = self.presentation.is_none() && !self.external_edit_blocked();
        let undo_label = self.document.undo_description().map_or_else(
            || "元に戻す".to_owned(),
            |description| format!("元に戻す: {description}"),
        );
        let redo_label = self.document.redo_description().map_or_else(
            || "やり直す".to_owned(),
            |description| format!("やり直す: {description}"),
        );
        let input = self.editor.read(cx);
        let input_focused = editing_enabled && input.has_focus(window);
        let can_copy = input_focused && input.selected_text().is_some();
        let undo_item = if input_focused {
            MenuItem::action("元に戻す: 文字編集", input::Undo).disabled(!input.can_undo())
        } else {
            MenuItem::action(undo_label, UndoEdit)
                .disabled(!editing_enabled || !self.document.can_undo())
        };
        let redo_item = if input_focused {
            MenuItem::action("やり直す: 文字編集", input::Redo).disabled(!input.can_redo())
        } else {
            MenuItem::action(redo_label, RedoEdit)
                .disabled(!editing_enabled || !self.document.can_redo())
        };
        cx.set_menus([Menu::new("編集").items([
            undo_item,
            redo_item,
            MenuItem::action("切り取り", input::Cut).disabled(!can_copy),
            MenuItem::action("コピー", input::Copy).disabled(!can_copy),
            MenuItem::action("貼り付け", input::Paste).disabled(!input_focused),
            MenuItem::action("すべてを選択", input::SelectAll).disabled(!input_focused),
        ])]);
        if let Some(model) = self.presentation.clone() {
            div()
                .size_full()
                .key_context("Presentation")
                .track_focus(&self.root_focus)
                .on_mouse_move(cx.listener(|this, _, window, cx| this.pointer_moved(window, cx)))
                .on_mouse_up(MouseButton::Left, cx.listener(|_, _, _, cx| cx.notify()))
                .on_action(cx.listener(Self::next_slide))
                .on_action(cx.listener(Self::previous_slide))
                .on_action(cx.listener(Self::exit_presentation))
                .child(self.render_canvas(&model, self.presentation_slide, false, window, cx))
                .into_any_element()
        } else {
            self.render_editor(window, cx).into_any_element()
        }
    }
}

impl Focusable for PsychoApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.root_focus.clone()
    }
}

fn outline_kind_label(element: &Element) -> &'static str {
    match element {
        Element::Heading(_) => "見出し",
        Element::Text(_) => "本文",
        Element::Bullets(_) => "箇条書き",
        Element::Code { .. } => "コード",
        Element::Image { .. } => "画像",
        Element::Columns { .. } => "2列",
    }
}

fn element_kind(element: &Element) -> ElementKind {
    match element {
        Element::Heading(_) => ElementKind::Heading,
        Element::Text(_) => ElementKind::Text,
        Element::Bullets(_) => ElementKind::Bullets,
        Element::Code { .. } => ElementKind::Code,
        Element::Image { .. } => ElementKind::Image,
        Element::Columns { .. } => ElementKind::Columns,
    }
}

fn nested_element(
    model: &PresentationModel,
    slide_index: usize,
    columns_index: usize,
    column_index: usize,
    element_index: usize,
) -> Option<&Element> {
    model
        .slides
        .get(slide_index)?
        .elements
        .get(columns_index)
        .and_then(|element| match element {
            Element::Columns { left, right, .. } => {
                (if column_index == 0 { left } else { right }).get(element_index)
            }
            _ => None,
        })
}

fn select_canvas_target(
    app: &mut PsychoApp,
    target: CanvasTarget,
    field: ElementField,
    event: &MouseUpEvent,
    window: &mut Window,
    cx: &mut Context<PsychoApp>,
) {
    if (app.canvas_editing || event.click_count < 2)
        && app.editor.read(cx).has_focus(window)
        && canvas_target_matches_edit_target(target, field, app.target)
    {
        return;
    }
    let edit = event.click_count >= 2;
    match target {
        CanvasTarget::Element { slide, index }
            if app
                .document
                .model()
                .and_then(|model| model.slides.get(slide))
                .and_then(|slide| slide.elements.get(index))
                .is_some_and(|element| matches!(element, Element::Columns { .. })) =>
        {
            app.select_columns(slide, index, window, cx);
        }
        CanvasTarget::Element { slide, index } => {
            app.select_element(slide, index, field, edit, window, cx);
        }
        CanvasTarget::NestedElement {
            slide,
            columns,
            column,
            index,
        } => app.select_nested_element(slide, columns, column, index, field, edit, window, cx),
    }
}

fn canvas_target_matches_edit_target(
    target: CanvasTarget,
    field: ElementField,
    selection: EditTarget,
) -> bool {
    match (target, selection) {
        (
            CanvasTarget::Element { slide, index },
            EditTarget::Element {
                slide: selected_slide,
                index: selected_index,
                field: selected_field,
            },
        ) => slide == selected_slide && index == selected_index && field == selected_field,
        (
            CanvasTarget::NestedElement {
                slide,
                columns,
                column,
                index,
            },
            EditTarget::NestedElement {
                slide: selected_slide,
                columns: selected_columns,
                column: selected_column,
                index: selected_index,
                field: selected_field,
            },
        ) => {
            slide == selected_slide
                && columns == selected_columns
                && column == selected_column
                && index == selected_index
                && field == selected_field
        }
        _ => false,
    }
}

fn canvas_source(target: CanvasTarget) -> CanvasSource {
    match target {
        CanvasTarget::Element { slide, index } => CanvasSource::Element { slide, index },
        CanvasTarget::NestedElement {
            slide,
            columns,
            column,
            index,
        } => CanvasSource::NestedElement {
            slide,
            columns,
            column,
            index,
        },
    }
}

fn canvas_target_id(target: CanvasTarget) -> String {
    match target {
        CanvasTarget::Element { slide, index } => format!("canvas-element-{slide}-{index}"),
        CanvasTarget::NestedElement {
            slide,
            columns,
            column,
            index,
        } => format!("canvas-element-{slide}-{columns}-{column}-{index}"),
    }
}

fn drop_destination(target: CanvasTarget) -> DropDestination {
    match target {
        CanvasTarget::Element { slide, index } => DropDestination::Element {
            slide,
            index,
            after: false,
        },
        CanvasTarget::NestedElement {
            slide,
            columns,
            column,
            index,
        } => DropDestination::NestedElement {
            slide,
            columns,
            column,
            index,
            after: false,
        },
    }
}

fn insertion_after_removal(from: usize, boundary: usize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let destination = if from < boundary {
        boundary.saturating_sub(1)
    } else {
        boundary
    };
    destination.min(count - 1)
}

fn nested_column_len(
    document: &PresentationDocument,
    slide: usize,
    columns: usize,
    column: usize,
) -> usize {
    document
        .model()
        .and_then(|model| model.slides.get(slide))
        .and_then(|slide| slide.elements.get(columns))
        .and_then(|element| match element {
            Element::Columns { left, right, .. } => {
                Some(if column == 0 { left.len() } else { right.len() })
            }
            _ => None,
        })
        .unwrap_or(0)
}

fn slides_have_same_members(before: &[psycho::Slide], after: &[psycho::Slide]) -> bool {
    if before.len() != after.len() {
        return false;
    }
    let mut remaining = after.iter().collect::<Vec<_>>();
    for slide in before {
        let Some(index) = remaining.iter().position(|candidate| *candidate == slide) else {
            return false;
        };
        remaining.swap_remove(index);
    }
    true
}

fn restored_element_after_history(
    before: &psycho::Slide,
    after: &psycho::Slide,
    slide: usize,
) -> Option<CanvasSource> {
    if let Some(index) = inserted_index(&before.elements, &after.elements) {
        return Some(CanvasSource::Element { slide, index });
    }
    for (columns, (before_element, after_element)) in before
        .elements
        .iter()
        .zip(after.elements.iter())
        .enumerate()
    {
        let (
            Element::Columns {
                left: before_left,
                right: before_right,
                ..
            },
            Element::Columns {
                left: after_left,
                right: after_right,
                ..
            },
        ) = (before_element, after_element)
        else {
            continue;
        };
        if let Some(index) = inserted_index(before_left, after_left) {
            return Some(CanvasSource::NestedElement {
                slide,
                columns,
                column: 0,
                index,
            });
        }
        if let Some(index) = inserted_index(before_right, after_right) {
            return Some(CanvasSource::NestedElement {
                slide,
                columns,
                column: 1,
                index,
            });
        }
    }
    None
}

fn inserted_index<T: Clone + PartialEq>(before: &[T], after: &[T]) -> Option<usize> {
    if after.len() != before.len() + 1 {
        return None;
    }
    (0..after.len()).find(|index| {
        after
            .iter()
            .enumerate()
            .filter(|(candidate, _)| candidate != index)
            .map(|(_, value)| value)
            .eq(before.iter())
    })
}

fn target_slide(target: EditTarget) -> Option<usize> {
    match target {
        EditTarget::Title => None,
        EditTarget::Element { slide, .. }
        | EditTarget::Columns { slide, .. }
        | EditTarget::ColumnWidth { slide, .. }
        | EditTarget::NestedElement { slide, .. } => Some(slide),
    }
}

fn validate_editor_value(target: EditTarget, value: &str) -> Result<(), DocumentError> {
    let reason = match target {
        EditTarget::Title if value.is_empty() => Some("タイトルを入力してください"),
        EditTarget::ColumnWidth { .. }
            if value
                .trim()
                .parse::<u8>()
                .ok()
                .is_none_or(|width| !(1..=99).contains(&width)) =>
        {
            Some("列幅は1〜99の整数で指定してください")
        }
        EditTarget::Element {
            field: ElementField::ImagePath,
            ..
        }
        | EditTarget::NestedElement {
            field: ElementField::ImagePath,
            ..
        } if value.is_empty() || Path::new(value).is_absolute() => {
            Some("画像参照は空でない相対パスで指定してください")
        }
        _ => None,
    };
    reason.map_or(Ok(()), |reason| {
        Err(DocumentError::UnsafeValue(reason.into()))
    })
}

fn apply_editor_value(
    document: &mut PresentationDocument,
    target: EditTarget,
    value: &str,
) -> Result<(), DocumentError> {
    validate_editor_value(target, value)?;
    // Optional fields can have an empty editor value while still being
    // absent in KDL. Selecting and ending an unchanged field must preserve
    // that representation as well as existing quoting and list boundaries.
    let element = document.model().and_then(|model| match target {
        EditTarget::Element { slide, index, .. } => model.slides.get(slide)?.elements.get(index),
        EditTarget::NestedElement {
            slide,
            columns,
            column,
            index,
            ..
        } => nested_element(model, slide, columns, column, index),
        _ => None,
    });
    let field = match target {
        EditTarget::Element { field, .. } | EditTarget::NestedElement { field, .. } => Some(field),
        _ => None,
    };
    if let Some((element, field)) = element.zip(field)
        && element_supports_field(element, field)
        && element_field_value(element, field) == value
    {
        return Ok(());
    }
    match target {
        EditTarget::Title => document.set_title(value),
        EditTarget::Columns { .. } => Ok(()),
        EditTarget::ColumnWidth { slide, index, left } => {
            let entered = value.trim().parse::<u8>().map_err(|_| {
                DocumentError::UnsafeValue("列幅は1〜99の整数で指定してください".into())
            })?;
            if !(1..=99).contains(&entered) {
                return Err(DocumentError::UnsafeValue(
                    "列幅は1〜99の整数で指定してください".into(),
                ));
            }
            let widths = document
                .model()
                .and_then(|model| model.slides.get(slide))
                .and_then(|slide| slide.elements.get(index))
                .and_then(|element| match element {
                    Element::Columns {
                        left_width,
                        right_width,
                        ..
                    } => Some((*left_width, *right_width)),
                    _ => None,
                })
                .ok_or(DocumentError::InvalidDocument)?;
            let (new_left, new_right) = if left {
                (entered, 100 - entered)
            } else {
                (100 - entered, entered)
            };
            if widths == (new_left, new_right) {
                return Ok(());
            }
            document.set_column_widths(slide, index, new_left, new_right)
        }
        EditTarget::Element {
            slide,
            index,
            field,
        } => {
            if field == ElementField::Text
                && matches!(
                    document
                        .model()
                        .and_then(|model| model.slides.get(slide))
                        .and_then(|slide| slide.elements.get(index)),
                    Some(Element::Bullets(_))
                )
            {
                document.set_bullets_text(slide, index, value)
            } else {
                document.set_element_field(slide, index, field, value)
            }
        }
        EditTarget::NestedElement {
            slide,
            columns,
            column,
            index,
            field,
        } => {
            if field == ElementField::Text
                && document
                    .model()
                    .and_then(|model| nested_element(model, slide, columns, column, index))
                    .is_some_and(|element| matches!(element, Element::Bullets(_)))
            {
                document.set_column_bullets_text(slide, columns, column, index, value)
            } else {
                document.set_column_element_field(slide, columns, column, index, field, value)
            }
        }
    }
}

fn element_supports_field(element: &Element, field: ElementField) -> bool {
    match (element, field) {
        (Element::Heading(_) | Element::Text(_), ElementField::Text) => true,
        (Element::Code { .. }, ElementField::Text | ElementField::CodeLanguage) => true,
        (Element::Bullets(_), ElementField::Text) => true,
        (Element::Bullets(items), ElementField::Bullet(index)) => index < items.len(),
        (Element::Image { .. }, ElementField::ImagePath | ElementField::Caption) => true,
        _ => false,
    }
}

fn default_edit_field(element: &Element) -> ElementField {
    match element {
        Element::Image { .. } => ElementField::ImagePath,
        Element::Heading(_) | Element::Text(_) | Element::Bullets(_) | Element::Code { .. } => {
            ElementField::Text
        }
        Element::Columns { .. } => ElementField::Text,
    }
}

fn element_field_value(element: &Element, field: ElementField) -> String {
    match (element, field) {
        (Element::Heading(text) | Element::Text(text), ElementField::Text) => text.clone(),
        (Element::Code { text, .. }, ElementField::Text) => text.clone(),
        (Element::Code { language, .. }, ElementField::CodeLanguage) => {
            language.clone().unwrap_or_default()
        }
        (Element::Image { path, .. }, ElementField::ImagePath) => path.clone(),
        (Element::Image { caption, .. }, ElementField::Caption) => {
            caption.clone().unwrap_or_default()
        }
        (Element::Bullets(items), ElementField::Text) => items.join("\n"),
        (Element::Bullets(items), ElementField::Bullet(item)) => {
            items.get(item).cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn load_asset_images(
    images: HashMap<String, image::RgbaImage>,
) -> HashMap<String, Arc<gpui::RenderImage>> {
    images
        .into_iter()
        .map(|(path, mut pixels)| {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            let frame = image::Frame::new(pixels);
            (path, Arc::new(gpui::RenderImage::new(vec![frame])))
        })
        .collect()
}

fn supports_canvas_text_editor(element: &Element, target: EditTarget) -> bool {
    let field = match target {
        EditTarget::Element { field, .. } | EditTarget::NestedElement { field, .. } => field,
        _ => return false,
    };
    match field {
        ElementField::Text => matches!(
            element,
            Element::Heading(_) | Element::Text(_) | Element::Bullets(_) | Element::Code { .. }
        ),
        ElementField::Caption => matches!(element, Element::Image { .. }),
        _ => false,
    }
}

fn render_inline_editor(
    element: &Element,
    scale: f32,
    width: f32,
    placement: &text::TextPlacement,
    editor: gpui::Entity<TextInputState>,
    cx: &mut App,
) -> gpui::Div {
    let (font_size, line_height) = match element {
        Element::Heading(_) => (48., 60.),
        Element::Code { .. } => (22., 30.8),
        _ => (28., 42.),
    };
    let code = matches!(element, Element::Code { .. });
    editor.update(cx, |input, _| {
        input.set_canvas_layout(Some(input::CanvasInputLayout {
            font_size,
            line_height,
            width: width - if code { 32. } else { 0. },
            scale,
            placement: if code {
                placement.shifted(16., 16.)
            } else {
                placement.clone()
            },
            code,
            language: match element {
                Element::Code { language, .. } => language.clone(),
                _ => None,
            },
            bullets: matches!(element, Element::Bullets(_)),
        }))
    });
    let base = div().w_full().text_color(rgb(0x222222));
    match element {
        Element::Heading(_) => base
            .text_size(px(48.0 * scale))
            .line_height(px(60.0 * scale))
            .font_weight(gpui::FontWeight::BOLD)
            .child(editor),
        Element::Text(_) | Element::Bullets(_) => base
            .text_size(px(28.0 * scale))
            .line_height(px(42.0 * scale))
            .child(editor),
        Element::Code { .. } => base
            .bg(rgb(0xf3f4f6))
            .p(px(16.0 * scale))
            .font_family("Menlo")
            .text_size(px(22.0 * scale))
            .line_height(px(30.8 * scale))
            .child(editor),
        Element::Image { .. } | Element::Columns { .. } => base,
    }
}

fn render_element(
    element: &Element,
    scale: f32,
    selected: bool,
    images: &HashMap<String, Arc<gpui::RenderImage>>,
    content_width: f32,
    window: &Window,
    placement: &text::TextPlacement,
) -> gpui::Div {
    let base = div()
        .w_full()
        .flex_shrink_0()
        .when(selected, |element| {
            element.border_1().border_color(rgb(0x3b82f6))
        })
        .text_color(rgb(0x222222));
    match element {
        Element::Heading(value) => base.child(text::shared_text(
            value,
            ".SystemUIFont",
            48.0,
            60.0,
            content_width / scale,
            true,
            Vec::new(),
            scale,
            window,
            placement,
        )),
        Element::Text(value) => base.child(text::shared_text(
            value,
            ".SystemUIFont",
            28.0,
            42.0,
            content_width / scale,
            false,
            Vec::new(),
            scale,
            window,
            placement,
        )),
        Element::Bullets(items) => {
            let mut list = div()
                .flex()
                .flex_col()
                .gap(px(8.0 * scale))
                .text_size(px(28.0 * scale))
                .line_height(px(42.0 * scale));
            let mut cursor_y = 0.;
            for item in items {
                list =
                    list.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .child(div().w(px(28.0 * scale)).flex_shrink_0().child(
                                text::shared_text(
                                    "•",
                                    ".SystemUIFont",
                                    28.0,
                                    42.0,
                                    f32::INFINITY,
                                    false,
                                    Vec::new(),
                                    scale,
                                    window,
                                    &placement.shifted(0., cursor_y),
                                ),
                            ))
                            .child(div().flex_1().child(text::shared_text(
                                item,
                                ".SystemUIFont",
                                28.0,
                                42.0,
                                content_width / scale - 28.0,
                                false,
                                Vec::new(),
                                scale,
                                window,
                                &placement.shifted(28., cursor_y),
                            ))),
                    );
                cursor_y += measure_text(
                    item,
                    ".SystemUIFont",
                    28.,
                    42.,
                    (content_width / scale - 28.).max(1.),
                    false,
                    window,
                )
                .0 + 8.;
            }
            base.when(items.is_empty(), |element| element.h(px(42.0 * scale)))
                .child(list)
        }
        Element::Code {
            text: value,
            language,
        } => {
            let expanded = text::expand_tabs(value);
            base.bg(rgb(0xf3f4f6))
                .p(px(16.0 * scale))
                .font_family("Menlo")
                .text_size(px(22.0 * scale))
                .line_height(px(30.8 * scale))
                .whitespace_nowrap()
                .child(text::shared_text(
                    &expanded,
                    "Menlo",
                    22.0,
                    30.8,
                    f32::INFINITY,
                    false,
                    code_highlights(&expanded, language.as_deref()),
                    scale,
                    window,
                    &placement.shifted(16., 16.),
                ))
        }
        Element::Image { path, caption } => {
            let image = if let Some(image) = images.get(path) {
                div()
                    .h(px(320.0 * scale))
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgb(0xffffff))
                    .child(
                        gpui::img(image.clone())
                            .object_fit(gpui::ObjectFit::Contain)
                            .w_full()
                            .h_full(),
                    )
            } else {
                div()
                    .h(px(320.0 * scale))
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgb(0xe5e7eb))
                    .text_size(px(18.0 * scale))
                    .child(format!("画像を読み込めません: {path}"))
            };
            let mut item = base.flex().flex_col().gap(px(12.0 * scale)).child(image);
            if let Some(caption) = caption.as_ref().filter(|caption| !caption.is_empty()) {
                item = item.child(
                    div()
                        .text_size(px(20.0 * scale))
                        .line_height(px(28.0 * scale))
                        .child(text::shared_text(
                            caption,
                            ".SystemUIFont",
                            20.0,
                            28.0,
                            content_width / scale,
                            false,
                            Vec::new(),
                            scale,
                            window,
                            &placement.shifted(0., 332.),
                        )),
                );
            }
            item
        }
        Element::Columns { .. } => unreachable!("columns use render_canvas_element"),
    }
}

fn code_highlights(
    text: &str,
    language: Option<&str>,
) -> Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> {
    let language = language.unwrap_or_default().to_ascii_lowercase();
    if !matches!(
        language.as_str(),
        "rust" | "rs" | "yaml" | "yml" | "json" | "toml"
    ) {
        return Vec::new();
    }

    let bytes = text.as_bytes();
    let mut highlights = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let style = if is_code_comment_start(bytes, index, &language) {
            index = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            Some(gpui::rgb(0x6b7280).into())
        } else if matches!(bytes[index], b'"' | b'\'')
            && (bytes[index] == b'"' || matches!(language.as_str(), "rust" | "rs"))
        {
            let quote = bytes[index];
            index += 1;
            let mut escaped = false;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    break;
                }
            }
            Some(gpui::rgb(0x15803d).into())
        } else if is_identifier_start(bytes[index]) {
            index += 1;
            while index < bytes.len() && is_identifier_continue(bytes[index]) {
                index += 1;
            }
            let token = &text[start..index];
            let is_yaml_key = matches!(language.as_str(), "yaml" | "yml" | "toml")
                && bytes[index..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_whitespace())
                    .count()
                    + index
                    < bytes.len()
                && bytes[index..]
                    .iter()
                    .skip_while(|byte| byte.is_ascii_whitespace())
                    .next()
                    == Some(&b':');
            let is_keyword = match language.as_str() {
                "rust" | "rs" => matches!(
                    token,
                    "as" | "async"
                        | "await"
                        | "break"
                        | "const"
                        | "continue"
                        | "crate"
                        | "dyn"
                        | "else"
                        | "enum"
                        | "extern"
                        | "false"
                        | "fn"
                        | "for"
                        | "if"
                        | "impl"
                        | "in"
                        | "let"
                        | "loop"
                        | "match"
                        | "mod"
                        | "move"
                        | "mut"
                        | "pub"
                        | "ref"
                        | "return"
                        | "self"
                        | "Self"
                        | "static"
                        | "struct"
                        | "super"
                        | "trait"
                        | "true"
                        | "type"
                        | "unsafe"
                        | "use"
                        | "where"
                        | "while"
                ),
                "yaml" | "yml" | "json" | "toml" => {
                    matches!(
                        token,
                        "true" | "false" | "null" | "yes" | "no" | "on" | "off"
                    )
                }
                _ => false,
            };
            if is_yaml_key {
                Some(gpui::rgb(0x1d4ed8).into())
            } else if is_keyword {
                Some(gpui::rgb(0x7c3aed).into())
            } else {
                None
            }
        } else if bytes[index].is_ascii_digit() {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.'))
            {
                index += 1;
            }
            Some(gpui::rgb(0xb45309).into())
        } else {
            index += 1;
            None
        };
        if let Some(style) = style.filter(|_| start < index) {
            highlights.push((start..index, style));
        }
    }
    highlights
}

fn is_code_comment_start(bytes: &[u8], index: usize, language: &str) -> bool {
    match language {
        "rust" | "rs" => bytes.get(index..index + 2) == Some(b"//"),
        "yaml" | "yml" | "toml" => bytes[index] == b'#',
        _ => false,
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn collect_layout_diagnostics(model: &PresentationModel, window: &Window) -> Vec<LayoutDiagnostic> {
    let content_width = 1280.0 - 64.0 * 2.0;
    let mut diagnostics = Vec::new();
    for (slide_index, slide) in model.slides.iter().enumerate() {
        let mut cursor_y = 48.0;
        for (element_index, element) in slide.elements.iter().enumerate() {
            if element_index > 0 {
                cursor_y += 24.0;
            }
            let mut reasons = Vec::new();
            let height = measure_element(element, content_width, window, &mut reasons);
            if cursor_y + height > 720.0 - 48.0 {
                reasons.push("内容がスライド下端を越えます");
            }
            if !reasons.is_empty() {
                reasons.sort_unstable();
                reasons.dedup();
                diagnostics.push(LayoutDiagnostic {
                    slide_index,
                    element_index,
                    message: reasons.join("、"),
                });
            }
            cursor_y += height;
        }
    }
    diagnostics
}

fn measure_element(
    element: &Element,
    width: f32,
    window: &Window,
    reasons: &mut Vec<&'static str>,
) -> f32 {
    match element {
        Element::Heading(text) => {
            measure_text(text, ".SystemUIFont", 48.0, 60.0, width, true, window).0
        }
        Element::Text(text) => {
            measure_text(text, ".SystemUIFont", 28.0, 42.0, width, false, window).0
        }
        Element::Bullets(items) => {
            if items.is_empty() {
                return 42.0;
            }
            let mut height = 0.0;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    height += 8.0;
                }
                height += measure_text(
                    item,
                    ".SystemUIFont",
                    28.0,
                    42.0,
                    (width - 28.0).max(1.0),
                    false,
                    window,
                )
                .0;
            }
            height
        }
        Element::Code { text, .. } => {
            let text = text::expand_tabs(text);
            let (height, natural_width) =
                measure_text(&text, "Menlo", 22.0, 30.8, f32::INFINITY, false, window);
            if natural_width > width - 32.0 {
                reasons.push("コードが横にはみ出します");
            }
            height + 32.0
        }
        Element::Image { caption, .. } => {
            let Some(caption) = caption.as_ref().filter(|caption| !caption.is_empty()) else {
                return 320.0;
            };
            320.0
                + 12.0
                + measure_text(caption, ".SystemUIFont", 20.0, 28.0, width, false, window).0
        }
        Element::Columns {
            left_width,
            right_width,
            left,
            right,
        } => {
            let inner_width = (width - 24.0).max(1.0);
            let left_width = inner_width * *left_width as f32 / 100.0;
            let right_width = inner_width * *right_width as f32 / 100.0;
            measure_element_list(left, left_width, window, reasons).max(measure_element_list(
                right,
                right_width,
                window,
                reasons,
            ))
        }
    }
}

fn measure_element_list(
    elements: &[Element],
    width: f32,
    window: &Window,
    reasons: &mut Vec<&'static str>,
) -> f32 {
    let mut height = 0.0;
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            height += 24.0;
        }
        height += measure_element(element, width, window, reasons);
    }
    height
}

fn measure_text(
    text: &str,
    family: &str,
    font_size: f32,
    line_height: f32,
    wrap_width: f32,
    bold: bool,
    window: &Window,
) -> (f32, f32) {
    let text_style = window.text_style();
    let mut font = text_style.font();
    font.family = family.into();
    if bold {
        font.weight = gpui::FontWeight::BOLD;
    }
    let run = gpui::TextRun {
        len: text.len(),
        font,
        color: text_style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let wrap_width = wrap_width.is_finite().then(|| px(wrap_width));
    let lines = window
        .text_system()
        .shape_text(text.into(), px(font_size), &[run], wrap_width, None)
        .unwrap_or_default();
    if lines.is_empty() {
        return (line_height, 0.0);
    }
    let height = lines
        .iter()
        .map(|line| f32::from(line.size(px(line_height)).height))
        .sum();
    let natural_width = lines
        .iter()
        .map(|line| f32::from(line.width()))
        .fold(0.0, f32::max);
    (height, natural_width)
}

fn diagnostics_view(
    diagnostics: &[Diagnostic],
    selected_source: Option<&Diagnostic>,
    cx: &mut Context<PsychoApp>,
) -> gpui::Div {
    let mut list = div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .min_w_0()
        .max_w(px(680.0))
        .whitespace_normal()
        .p_6()
        .bg(rgb(0xffffff));
    if diagnostics.is_empty() {
        list = list.child("Presentation を読み込めませんでした。");
    }
    for diagnostic in diagnostics {
        let kind = match diagnostic.kind {
            DiagnosticKind::Syntax => "KDL 構文",
            DiagnosticKind::Schema => "Schema",
        };
        let file_name = diagnostic.file_name.as_deref().unwrap_or("Presentation");
        let slide = diagnostic
            .slide_id
            .as_deref()
            .map(|id| format!("Slide id={id}"))
            .or_else(|| {
                diagnostic
                    .slide_index
                    .map(|index| format!("Slide {}", index + 1))
            });
        let mut row = div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_1()
            .border_color(rgb(0xe5e7eb))
            .bg(if selected_source == Some(diagnostic) {
                rgb(0xfff7ed)
            } else {
                rgb(0xffffff)
            })
            .text_color(rgb(0x991b1b))
            .child(format!(
                "{kind} · {file_name}:{}:{}{}",
                diagnostic.line,
                diagnostic.column,
                slide.map_or_else(String::new, |slide| format!(" · {slide}"))
            ))
            .child(diagnostic.message.clone())
            .child(
                div()
                    .font_family("Menlo")
                    .text_size(px(13.0))
                    .child(diagnostic.source_line.clone()),
            );
        let target = diagnostic.clone();
        row = row.cursor_pointer().on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| {
                this.navigate_to_diagnostic(target.clone(), window, cx)
            }),
        );
        list = list.child(row);
    }
    if let Some(diagnostic) = selected_source {
        let file_name = diagnostic.file_name.as_deref().unwrap_or("Presentation");
        let line = if diagnostic.source_line.is_empty() {
            "(空行)".to_owned()
        } else {
            diagnostic.source_line.clone()
        };
        list = list.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .mt_2()
                .p_2()
                .border_1()
                .border_color(rgb(0x93c5fd))
                .bg(rgb(0xeff6ff))
                .child(format!(
                    "読み取り専用のソース行 · {file_name}:{}:{}",
                    diagnostic.line, diagnostic.column
                ))
                .child(div().font_family("Menlo").text_size(px(13.0)).child(line))
                .child(
                    div()
                        .font_family("Menlo")
                        .text_size(px(13.0))
                        .text_color(rgb(0x2563eb))
                        .child(format!(
                            "{}^",
                            " ".repeat(diagnostic.column.saturating_sub(1))
                        )),
                ),
        );
    }
    list
}

fn button(
    cx: &mut Context<PsychoApp>,
    label: impl Into<gpui::SharedString>,
    enabled: bool,
    handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(13.0))
        .bg(if enabled {
            rgb(0xffffff)
        } else {
            rgb(0xe5e7eb)
        })
        .border_1()
        .border_color(rgb(0xb8bdc5))
        .cursor_pointer()
        .child(label.into())
        // A toolbar press must not focus the Editor ancestor before its
        // handler decides whether Undo belongs to the active text field.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, event, window, cx| {
                if enabled {
                    handler(this, event, window, cx);
                }
            }),
        )
}

fn main() {
    application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-s", SaveFile, Some("Editor")),
            KeyBinding::new("cmd-shift-s", SaveAs, Some("Editor")),
            KeyBinding::new("cmd-z", UndoEdit, Some("Editor")),
            KeyBinding::new("cmd-shift-z", RedoEdit, Some("Editor")),
            KeyBinding::new("delete", DeleteSelection, Some("Editor")),
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
            KeyBinding::new("cmd-c", input::Copy, Some("TextInput")),
            KeyBinding::new("cmd-x", input::Cut, Some("TextInput")),
            KeyBinding::new("cmd-v", input::Paste, Some("TextInput")),
            KeyBinding::new("tab", input::Tab, Some("TextInput")),
            KeyBinding::new("cmd-z", input::Undo, Some("TextInput")),
            KeyBinding::new("cmd-shift-z", input::Redo, Some("TextInput")),
            KeyBinding::new("enter", input::Enter, Some("TextInput")),
            KeyBinding::new("escape", input::Escape, Some("TextInput")),
        ]);
        let document = match std::env::args_os().nth(1) {
            Some(path) => PresentationDocument::open(path).expect("open requested presentation"),
            None => {
                PresentationDocument::from_source_with_asset_base(
                    DEMO,
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples"),
                )
                    .expect("load built-in example")
            }
        };
        let title = document
            .model()
            .map_or_else(String::new, |model| model.title.clone());
        let bounds = Bounds::centered(None, WINDOW_SIZE, cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let editor = cx.new(|cx| TextInputState::new(cx, &title));
                    let view = cx.new(|cx| PsychoApp::new(cx, document, editor, window));
                    let weak_view = view.downgrade();
                    let weak_watch = view.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        weak_view
                            .update(cx, |this, cx| {
                                if let Err(error) = this.commit_draft(window, cx) {
                                    this.pending_close = true;
                                    this.status = format!(
                                        "編集内容を確定できないため、保存・破棄・キャンセルを選んでください: {error}"
                                    );
                                    window.focus(&this.root_focus, cx);
                                    cx.notify();
                                    return false;
                                }
                                if this.document.is_dirty() {
                                    this.pending_close = true;
                                    this.status = "未保存の編集があります。保存・破棄・キャンセルを選んでください。"
                                        .into();
                                    window.focus(&this.root_focus, cx);
                                    cx.notify();
                                    false
                                } else {
                                    true
                                }
                            })
                            .unwrap_or(true)
                    });
                    window
                        .spawn(cx, async move |cx| {
                            loop {
                                cx.background_executor()
                                    .timer(Duration::from_millis(750))
                                    .await;
                                let alive = cx
                                    .update(|window, cx| {
                                        weak_watch
                                            .update(cx, |this, cx| {
                                                this.poll_external_change(window, cx);
                                                true
                                            })
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                                if !alive {
                                    break;
                                }
                            }
                        })
                        .detach();
                    view
                },
            )
            .expect("open PSYCHO window");
        window
            .update(cx, |view, window, cx| {
                window.focus(&view.root_focus, cx);
                cx.activate(true);
            })
            .expect("activate PSYCHO window");
    });
}
