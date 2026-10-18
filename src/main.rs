#![allow(unexpected_cfgs)]

mod editor_actions;
mod editor_insert;
mod editor_validation;
mod editor_view;
mod ime;
mod input;
mod navigator;
mod text;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use editor_actions::*;
use editor_insert::{ColumnSide, InsertElement, InsertMenuState, InsertParent};
use editor_view::{EditorPalette, EditorViewState, Pane, PaneResize, Popup, ZoomMode};
use gpui::{
    App, Bounds, Context, DragMoveEvent, FocusHandle, Focusable, InteractiveElement, KeyBinding,
    Menu, MenuItem, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use input::TextInputState;
use psycho::{
    AssetDiagnostic, Diagnostic, DiagnosticKind, DocumentError, Element, ElementField, ElementKind,
    ExternalState, LayoutDiagnostic, PresentationDocument, PresentationModel, PresentationSession,
    PresentationStart, PresentationStartError, SlidePosition,
};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

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

struct InspectorControl {
    focus: FocusHandle,
    anchor: gpui::ScrollAnchor,
    _subscription: gpui::Subscription,
}

struct OutlineRow {
    source: CanvasSource,
    label: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SlideInteraction {
    Editor,
    ReadOnly,
}

enum DocumentAction {
    Open(PathBuf),
    Close,
    Reload,
}
#[derive(Clone, Copy)]
enum SaveIntent {
    Save,
    SaveAs,
}
enum DocumentFlow {
    Idle,
    Active { return_focus: Option<FocusHandle> },
}
struct SaveFailure {
    intent: SaveIntent,
    message: String,
}

struct PsychoApp {
    view: EditorViewState,
    palette: EditorPalette,
    popup_return_focus: Option<FocusHandle>,
    document: PresentationDocument,
    editor: gpui::Entity<TextInputState>,
    root_focus: FocusHandle,
    canvas_focus: FocusHandle,
    navigator: navigator::NavigatorState,
    ime_geometry: ime::Geometry,
    current_slide: usize,
    outline_scroll: gpui::ScrollHandle,
    inspector_scroll: gpui::ScrollHandle,
    outline_exposure: Option<(usize, Option<CanvasSource>)>,
    outline_focus: Vec<(FocusHandle, gpui::Subscription)>,
    target: EditTarget,
    canvas_editing: bool,
    presentation: Option<GuiPresentation>,
    presentation_cursor_hidden: bool,
    cursor_hide_generation: u64,
    status: String,
    document_flow: DocumentFlow,
    save_failure: Option<SaveFailure>,
    source_selection: Option<Diagnostic>,
    layout_diagnostics: Vec<LayoutDiagnostic>,
    asset_diagnostics: Vec<AssetDiagnostic>,
    asset_images: HashMap<String, Arc<gpui::RenderImage>>,
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

struct GuiPresentation {
    session: PresentationSession,
    render_images: HashMap<String, Arc<gpui::RenderImage>>,
    editor_was_fullscreen: bool,
}

impl PsychoApp {
    fn availability(&self, window: &Window, cx: &Context<Self>) -> EditorAvailability {
        let model = self.document.model();
        let editor = self.editor.read(cx);
        let view = self.presentation.is_none() && matches!(self.document_flow, DocumentFlow::Idle);
        let edit = view && model.is_some() && !self.external_edit_blocked();
        let count = model.map_or(0, |model| model.slides.len());
        let present_reason = if !edit {
            Some("文書や外部ファイルの問題を解決してください。")
        } else if count == 0 {
            Some("発表するスライドがありません。")
        } else if self.has_uncommitted_draft(cx) {
            None
        } else if !self.asset_diagnostics.is_empty() {
            Some("画像の問題を修正してください。")
        } else if !self.layout_diagnostics.is_empty() {
            Some("スライド内に収まらない内容を修正してください。")
        } else {
            None
        };
        EditorAvailability {
            open: view,
            save: edit,
            save_as: view && model.is_some(),
            edit,
            undo: edit
                && if editor.has_focus(window) {
                    editor.can_undo()
                } else {
                    self.document.can_undo()
                },
            redo: edit
                && if editor.has_focus(window) {
                    editor.can_redo()
                } else {
                    self.document.can_redo()
                },
            delete_slide: edit && count > 0,
            earlier: edit && self.current_slide > 0,
            later: edit && self.current_slide + 1 < count,
            present: present_reason.is_none(),
            present_reason,
            view,
        }
    }

    fn execute_action<A: EditorAction>(
        &mut self,
        action: &A,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !action.enabled(self.availability(window, cx)) {
            return;
        }
        let popup = self.view.popup.take();
        if popup.is_some() {
            self.restore_popup_focus(window, cx);
        }
        action.execute(self, window, cx);
    }

    fn action_button<A: EditorAction>(
        &self,
        action: A,
        compact: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let available = self.availability(window, cx);
        let enabled = action.enabled(available);
        let tooltip: gpui::SharedString = action.disabled_reason(available).map_or_else(
            || A::LABEL.into(),
            |reason| format!("{} · {reason}", A::LABEL).into(),
        );
        let palette = self.palette;
        let click_action = action.clone();
        let a11y_action = action.clone();
        let weak = cx.entity().downgrade();
        let mut button = div()
            .id(A::LABEL)
            .accessibility_id(A::LABEL)
            .focusable()
            .tab_stop(enabled)
            .role(gpui::Role::Button)
            .aria_label(A::LABEL)
            .a11y_synthetic_children(move |builder| {
                if !enabled {
                    builder.parent_node().set_disabled();
                }
            })
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .min_h(px(32.))
            .px(px(8.))
            .rounded_md()
            .text_size(px(13.))
            .bg(palette.surface)
            .text_color(if enabled {
                palette.text
            } else {
                palette.secondary
            })
            .border_1()
            .border_color(palette.border)
            .focus(|style| style.border_2().border_color(palette.focus))
            .tooltip(move |_, cx| cx.new(|_| EditorTooltip(tooltip.clone())).into())
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.execute_action(&click_action, window, cx)
            }))
            .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                weak.update(cx, |this, cx| this.execute_action(&a11y_action, window, cx))
                    .ok();
            });
        if let Some(icon) = A::ICON {
            button = button.child(gpui::svg().path(icon).size(px(20.)).text_color(if enabled {
                palette.text
            } else {
                palette.secondary
            }));
        }
        if !compact || A::ICON.is_none() {
            button = button.child(A::LABEL);
        }
        button.into_any_element()
    }

    fn show_popup(&mut self, popup: Popup, window: &mut Window, cx: &mut Context<Self>) {
        self.popup_return_focus = window.focused(cx);
        if !matches!(popup, Popup::Diagnostics | Popup::Present) {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = error;
                cx.notify();
                return;
            }
        }
        self.view.popup = Some(popup);
        window.focus(&self.root_focus, cx);
        cx.notify();
    }
    fn restore_popup_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self.popup_return_focus.take() {
            window.focus(&focus, cx);
        }
    }
    fn prepare_view_change(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.external_edit_blocked() {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = error;
                cx.notify();
                return false;
            }
        }
        true
    }
    fn toggle_pane(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        let validate =
            pane == Pane::Navigator || (self.view.inspector.visible && !self.canvas_editing);
        if validate && !self.prepare_view_change(window, cx) {
            return;
        }
        if pane == Pane::Inspector && self.view.inspector.visible && !self.canvas_editing {
            window.focus(&self.root_focus, cx);
        }
        let pane = self.view.pane(pane);
        pane.visible = !pane.visible;
        cx.notify();
    }
    fn change_pane_width(&mut self, pane: Pane, delta: f32, cx: &mut Context<Self>) {
        let state = self.view.pane(pane);
        let (min, max) = match pane {
            Pane::Navigator => (120., 360.),
            Pane::Inspector => (220., 440.),
        };
        state.preferred_width = (state.preferred_width + delta).clamp(min, max);
        cx.notify();
    }
    fn render_pane_resizer(
        &self,
        pane: Pane,
        width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(match pane {
                Pane::Navigator => "navigator-resizer",
                Pane::Inspector => "inspector-resizer",
            })
            .w(px(8.))
            .h_full()
            .flex_shrink_0()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .border_l_1()
            .border_color(self.palette.border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.prevent_default();
                    this.view.resize = Some(PaneResize {
                        pane,
                        start_x: f32::from(event.position.x),
                        start_width: width,
                    });
                    cx.notify();
                }),
            )
            .into_any_element()
    }
    fn update_pane_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(resize) = self.view.resize {
            let delta = (f32::from(event.position.x) - resize.start_x)
                * if resize.pane == Pane::Navigator {
                    1.
                } else {
                    -1.
                };
            let pane = self.view.pane(resize.pane);
            let (min, max) = if resize.pane == Pane::Navigator {
                (120., 360.)
            } else {
                (220., 440.)
            };
            pane.preferred_width = (resize.start_width + delta).clamp(min, max);
            cx.notify();
        }
    }

    fn new(
        cx: &mut Context<Self>,
        document: PresentationDocument,
        editor: gpui::Entity<TextInputState>,
        window: &mut Window,
    ) -> Self {
        let ime_rect = ime::Geometry::default();
        let input_ime_rect = ime_rect.clone();
        cx.observe_in(&editor, window, move |_, _, window, cx| {
            input_ime_rect.refresh(window, false);
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
            this.ime_geometry.refresh(window, false);
            if matches!(this.document_flow, DocumentFlow::Idle)
                && !matches!(this.view.popup, Some(Popup::Diagnostics | Popup::Present))
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
        cx.set_global(EditorPalette::for_appearance(window.appearance()));
        let current_title = document
            .model()
            .map_or_else(String::new, |model| model.title.clone());
        editor.update(cx, |input, cx| input.set_value(&current_title, cx));
        let assets = document.load_assets();
        let asset_diagnostics = assets.diagnostics;
        let asset_images = load_asset_images(assets.images);
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();
        Self {
            view: EditorViewState::default(),
            palette: EditorPalette::for_appearance(window.appearance()),
            popup_return_focus: None,
            document,
            editor,
            root_focus: cx.focus_handle(),
            canvas_focus: cx.focus_handle().tab_stop(true),
            navigator: navigator::NavigatorState::new(cx.focus_handle().tab_stop(true)),
            ime_geometry: ime_rect,
            current_slide: 0,
            outline_scroll: gpui::ScrollHandle::new(),
            inspector_scroll: gpui::ScrollHandle::new(),
            outline_exposure: None,
            outline_focus: Vec::new(),
            target: EditTarget::Title,
            canvas_editing: false,
            presentation: None,
            presentation_cursor_hidden: false,
            cursor_hide_generation: 0,
            status: "KDL ファイルを開いて編集できます。".into(),
            document_flow: DocumentFlow::Idle,
            save_failure: None,
            source_selection: None,
            layout_diagnostics: Vec::new(),
            asset_diagnostics,
            asset_images,
            column_resize_drag: None,
            drop_destination: None,
        }
    }

    fn external_edit_blocked(&self) -> bool {
        !matches!(self.document_flow, DocumentFlow::Idle)
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
        self.canvas_editing = false;
        let value = self.edit_value(self.target);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&value, cx);
        });
        window.focus(&self.root_focus, cx);
        cx.notify();
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
        let canvas_editing = edit
            && self
                .document
                .model()
                .and_then(|model| model.slides.get(slide))
                .and_then(|slide| slide.elements.get(index))
                .is_some_and(|element| supports_canvas_text_editor(element, target));
        if self.target == target {
            if self.canvas_editing && !edit {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
            let surface_changed = self.canvas_editing != canvas_editing;
            self.canvas_editing = canvas_editing;
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
        self.canvas_editing = canvas_editing;
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
        let canvas_editing = edit
            && self
                .document
                .model()
                .and_then(|model| nested_element(model, slide, columns, column, index))
                .is_some_and(|element| supports_canvas_text_editor(element, target));
        if self.target == target {
            if self.canvas_editing && !edit {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
            let surface_changed = self.canvas_editing != canvas_editing;
            self.canvas_editing = canvas_editing;
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
        self.canvas_editing = canvas_editing;
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
        self.canvas_editing = false;
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
        self.canvas_editing = false;
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

    #[allow(unexpected_cfgs, unused_variables)]
    fn finish_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.editor.read(cx).is_marked() {
            return true;
        }
        #[cfg(target_os = "macos")]
        {
            use objc::{msg_send, runtime::Object, sel, sel_impl};

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
            return !self.editor.read(cx).is_marked();
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.editor
                .update(cx, |input, cx| input.finish_composition(cx));
            !self.editor.read(cx).is_marked()
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_save(SaveIntent::Save, window, cx);
    }
    fn save_as_action(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_action(&SaveAs, window, cx);
    }
    fn save_action(&mut self, _: &SaveFile, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_action(&SaveFile, window, cx);
    }
    fn prompt_save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_save(SaveIntent::SaveAs, window, cx);
    }
    fn request_save(&mut self, intent: SaveIntent, window: &mut Window, cx: &mut Context<Self>) {
        self.start_document_flow(None, Some(intent), window, cx);
    }
    fn request_document_action(
        &mut self,
        action: DocumentAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_document_flow(Some(action), None, window, cx);
    }
    fn finish_document_flow(
        &mut self,
        restore_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let DocumentFlow::Active {
            return_focus: Some(focus),
        } = std::mem::replace(&mut self.document_flow, DocumentFlow::Idle)
        {
            if restore_focus {
                window.focus(&focus, cx);
            }
        }
        cx.notify();
    }
    fn start_document_flow(
        &mut self,
        action: Option<DocumentAction>,
        mut save: Option<SaveIntent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.document_flow, DocumentFlow::Idle) {
            return;
        }
        self.document_flow = DocumentFlow::Active {
            return_focus: window.focused(cx),
        };
        let reload = matches!(action, Some(DocumentAction::Reload));
        let confirmation =
            if action.is_some() && (self.document.is_dirty() || self.has_uncommitted_draft(cx)) {
                Some(if reload {
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        "未保存の編集を破棄して再読み込みしますか。",
                        Some("編集を残す場合はキャンセルし、別名保存してください。"),
                        &[
                            gpui::PromptButton::ok("破棄して再読み込み"),
                            gpui::PromptButton::cancel("キャンセル"),
                        ],
                        cx,
                    )
                } else {
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        "未保存の編集があります。",
                        Some("保存して続けるか、編集を破棄するか選んでください。"),
                        &[
                            gpui::PromptButton::ok("保存"),
                            gpui::PromptButton::new("破棄"),
                            gpui::PromptButton::cancel("キャンセル"),
                        ],
                        cx,
                    )
                })
            } else {
                None
            };
        let view = cx.entity();
        window
            .spawn(cx, async move |cx| {
                if let Some(confirmation) = confirmation {
                    match confirmation.await {
                        Ok(0) if reload => {}
                        Ok(0) => save = Some(SaveIntent::Save),
                        Ok(1) if !reload => {}
                        _ => {
                            let _ = cx.update(|window, cx| {
                                view.update(cx, |this, cx| {
                                    this.finish_document_flow(true, window, cx)
                                })
                            });
                            return;
                        }
                    }
                }
                while let Some(mut intent) = save {
                    let needs_as = cx
                        .update(|_, cx| {
                            view.read(cx).document.path().is_none()
                                || !matches!(
                                    view.read(cx).document.external_state(),
                                    ExternalState::Current
                                )
                        })
                        .unwrap_or(true);
                    if needs_as {
                        intent = SaveIntent::SaveAs;
                    }
                    let result = if matches!(intent, SaveIntent::SaveAs) {
                        let notice = cx.update(|window, cx| {
                            window.prompt(
                                gpui::PromptLevel::Warning,
                                "退避保存すると Undo / Redo 履歴が消去されます。",
                                Some("別フォルダーでは、編集していない画像参照も更新されます。"),
                                &[
                                    gpui::PromptButton::ok("退避先を選ぶ"),
                                    gpui::PromptButton::cancel("キャンセル"),
                                ],
                                cx,
                            )
                        });
                        let Ok(notice) = notice else {
                            return;
                        };
                        if !matches!(notice.await, Ok(0)) {
                            break;
                        }
                        let receiver = cx.update(|_, cx| {
                            let directory = view
                                .read(cx)
                                .document
                                .path()
                                .and_then(Path::parent)
                                .unwrap_or(Path::new("."))
                                .to_path_buf();
                            cx.prompt_for_new_path(&directory, Some("presentation.kdl"))
                        });
                        let Ok(receiver) = receiver else {
                            return;
                        };
                        let Ok(Ok(Some(path))) = receiver.await else {
                            break;
                        };
                        cx.update(|window, cx| {
                            view.update(cx, |this, cx| {
                                let result = this.save_as_with_draft(&path, window, cx);
                                if result.is_ok() {
                                    this.refresh_assets();
                                    let value = this.edit_value(this.target);
                                    this.editor
                                        .update(cx, |input, cx| input.set_value(&value, cx));
                                }
                                result
                            })
                        })
                    } else {
                        cx.update(|window, cx| {
                            view.update(cx, |this, cx| {
                                this.commit_draft(window, cx).and_then(|()| {
                                    this.document.save().map_err(|error| error.to_string())
                                })
                            })
                        })
                    };
                    let Ok(result) = result else {
                        return;
                    };
                    match result {
                        Ok(()) => {
                            let _ = cx.update(|_, cx| {
                                view.update(cx, |this, cx| {
                                    this.save_failure = None;
                                    this.source_selection = None;
                                    this.status = "保存しました。".into();
                                    cx.notify();
                                })
                            });
                            save = None;
                        }
                        Err(message) => {
                            let retry = cx.update(|window, cx| {
                                view.update(cx, |this, cx| {
                                    this.save_failure = Some(SaveFailure {
                                        intent,
                                        message: message.clone(),
                                    });
                                    cx.notify();
                                });
                                window.prompt(
                                    gpui::PromptLevel::Critical,
                                    "保存できませんでした。",
                                    Some(&message),
                                    &[
                                        gpui::PromptButton::ok("再試行"),
                                        gpui::PromptButton::cancel("キャンセル"),
                                    ],
                                    cx,
                                )
                            });
                            let Ok(retry) = retry else {
                                return;
                            };
                            if !matches!(retry.await, Ok(0)) {
                                break;
                            }
                        }
                    }
                }
                let _ = cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        let restore_focus = save.is_some() || action.is_none();
                        if save.is_none() {
                            match action {
                                Some(DocumentAction::Open(path)) => {
                                    this.open_document(path, window, cx)
                                }
                                Some(DocumentAction::Close) => {
                                    window.remove_window();
                                    return;
                                }
                                Some(DocumentAction::Reload) => {
                                    let position = this.reload_slide_position();
                                    match this.document.reload_from_disk() {
                                        Ok(()) => this.did_reload(position, window, cx),
                                        Err(_) => this.show_external_problem(window, cx),
                                    }
                                }
                                None => {}
                            }
                        }
                        this.finish_document_flow(restore_focus, window, cx);
                    })
                });
            })
            .detach();
        cx.notify();
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
        if !matches!(self.document_flow, DocumentFlow::Idle) {
            return;
        }
        self.document_flow = DocumentFlow::Active {
            return_focus: window.focused(cx),
        };
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("KDL Presentation を開く".into()),
        });
        let view = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let selected = receiver.await;
                let _ = cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        let focus =
                            match std::mem::replace(&mut this.document_flow, DocumentFlow::Idle) {
                                DocumentFlow::Active { return_focus } => return_focus,
                                DocumentFlow::Idle => None,
                            };
                        if let Some(focus) = focus.as_ref() {
                            window.focus(focus, cx);
                        }
                        if let Ok(Ok(Some(paths))) = selected {
                            if let Some(path) = paths.into_iter().next() {
                                this.request_open(path, window, cx);
                            }
                        }
                        cx.notify();
                    })
                });
            })
            .detach();
        cx.notify();
    }

    fn request_open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.request_document_action(DocumentAction::Open(path), window, cx);
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
        cx.notify();
    }

    fn reset_document_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_assets();
        self.current_slide = 0;
        self.target = EditTarget::Title;
        self.source_selection = None;
        self.save_failure = None;
        self.column_resize_drag = None;
        self.drop_destination = None;
        let title = self.edit_value(EditTarget::Title);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&title, cx);
        });
        window.focus(&self.root_focus, cx);
    }

    fn reload_slide_position(&self) -> SlidePosition {
        self.document
            .model()
            .map(|model| SlidePosition::capture(model, self.current_slide))
            .unwrap_or_default()
    }

    fn did_reload(&mut self, position: SlidePosition, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_document_view(window, cx);
        self.current_slide = self
            .document
            .model()
            .and_then(|model| position.resolve(model))
            .unwrap_or(0);
        self.status = "外部ファイルを再読み込みしました。".into();
    }

    fn reload_external_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_document_action(DocumentAction::Reload, window, cx);
    }

    fn add_slide(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let navigator_focused = self.navigator.focused(window);
        if let Err(error) = self.document.check_external_change() {
            self.status = format!("外部変更があるためスライド操作を取り消しました: {error}");
            cx.notify();
            return;
        }

        if self.external_edit_blocked() {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx).and_then(|()| {
            self.document
                .insert_slide(
                    self.document
                        .model()
                        .map_or(0, |model| (self.current_slide + 1).min(model.slides.len())),
                )
                .map(|index| self.current_slide = index)
                .map_err(|error| error.to_string())
        }) {
            self.status = format!("Slide を追加できませんでした: {error}");
        } else {
            self.reset_editor_to_title(cx);
            if navigator_focused {
                window.focus(&self.navigator.focus_handle(), cx);
            } else {
                window.focus(&self.root_focus, cx);
            }
            self.status = "Slide を追加しました。".into();
        }
        cx.notify();
    }

    fn delete_slide(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let navigator_focused = self.navigator.focused(window);
        if let Err(error) = self.document.check_external_change() {
            self.status = format!("外部変更があるためスライド操作を取り消しました: {error}");
            cx.notify();
            return;
        }

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
            if navigator_focused {
                window.focus(&self.navigator.focus_handle(), cx);
            } else {
                window.focus(&self.root_focus, cx);
            }
            self.status = "Slide を削除しました。".into();
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
        self.move_slide_to(from, to, window, cx);
    }

    fn move_slide_to(
        &mut self,
        from: usize,
        to: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let navigator_focused = self.navigator.focused(window);
        if let Err(error) = self.document.check_external_change() {
            self.status = format!("外部変更があるためスライド操作を取り消しました: {error}");
            cx.notify();
            return;
        }

        if from == to {
            return;
        }
        if self.external_edit_blocked() {
            self.status = "外部変更との競合があるためスライドを移動できません。".into();
            cx.notify();
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
        self.reset_editor_to_title(cx);
        if navigator_focused {
            window.focus(&self.navigator.focus_handle(), cx);
        } else {
            window.focus(&self.root_focus, cx);
        }
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
        self.execute_action(&UndoEdit, window, cx);
    }

    fn redo(&mut self, _: &RedoEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.execute_action(&RedoEdit, window, cx);
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
        if matches!(self.document.external_state(), ExternalState::Invalid(_)) {
            return;
        }
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
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            return;
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
        self.view.popup = None;
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
        #[cfg(target_os = "macos")]
        {
            use objc::{msg_send, sel, sel_impl};
            unsafe {
                let _: () = msg_send![objc::class!(NSCursor), hide];
            }
        }
        self.presentation_cursor_hidden = true;
    }

    fn show_presentation_cursor(&mut self) {
        if !self.presentation_cursor_hidden {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            use objc::{msg_send, sel, sel_impl};
            unsafe {
                let _: () = msg_send![objc::class!(NSCursor), unhide];
            }
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
        let position = self.reload_slide_position();
        let source = self.document.source().to_owned();
        let start = if start_at_current {
            PresentationStart::Current(self.current_slide)
        } else {
            PresentationStart::First
        };
        let result = self
            .document
            .start_presentation(start, |model| collect_layout_diagnostics(model, window));
        if self.document.source() != source {
            self.did_reload(position, window, cx);
        }
        let session = match result {
            Ok(session) => session,
            Err(error) => {
                self.status = error.to_string();
                match error {
                    PresentationStartError::Layout(issues) => self.layout_diagnostics = issues,
                    PresentationStartError::Assets(issues) => self.asset_diagnostics = issues,
                    PresentationStartError::Document(_) => self.show_external_problem(window, cx),
                    PresentationStartError::Empty => {}
                }
                cx.notify();
                return;
            }
        };
        self.layout_diagnostics.clear();
        self.asset_diagnostics.clear();
        let render_images = load_asset_images(session.images().clone());
        let editor_was_fullscreen = window.is_fullscreen();
        self.presentation = Some(GuiPresentation {
            session,
            render_images,
            editor_was_fullscreen,
        });
        self.status = "発表中です。Escape で編集へ戻ります。".into();
        if !editor_was_fullscreen {
            window.toggle_fullscreen();
        }
        self.schedule_cursor_hide(window, cx);
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    fn next_slide(&mut self, _: &NextSlide, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(presentation) = self.presentation.as_mut() {
            presentation.session.next();
        }
        cx.notify();
    }

    fn previous_slide(&mut self, _: &PreviousSlide, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(presentation) = self.presentation.as_mut() {
            presentation.session.previous();
        }
        cx.notify();
    }

    fn exit_presentation(
        &mut self,
        _: &ExitPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(presentation) = self.presentation.take() else {
            return;
        };
        self.current_slide = presentation
            .session
            .position()
            .resolve(self.document.model().unwrap())
            .unwrap_or(0);
        self.target = EditTarget::Title;
        self.canvas_editing = false;
        let title = self.edit_value(EditTarget::Title);
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(&title, cx);
        });
        self.cursor_hide_generation = self.cursor_hide_generation.wrapping_add(1);
        self.show_presentation_cursor();
        if !presentation.editor_was_fullscreen && window.is_fullscreen() {
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
        if value == self.edit_value(self.target) {
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
        let model = self.preview_model(cx).map(Arc::new);
        self.palette = EditorPalette::for_appearance(window.appearance());
        let palette = self.palette;
        cx.set_global(palette);
        let title = model
            .as_ref()
            .map_or("新しい Presentation", |model| model.title.as_str())
            .to_owned();
        let dirty = self.document.is_dirty() || self.has_uncommitted_draft(cx);
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(12.))
            .py(px(8.))
            .bg(palette.surface)
            .text_size(px(13.))
            .child(
                div()
                    .flex_shrink(1.)
                    .max_w(px(420.))
                    .text_ellipsis()
                    .min_w_0()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(
                        self.document
                            .path()
                            .and_then(Path::file_name)
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| title.clone()),
                    ),
            )
            .child(div().text_color(palette.secondary).child(if dirty {
                "未保存"
            } else if self.document.path().is_none() {
                "保存先未設定"
            } else {
                "保存済み"
            }));
        if let Some(failure) = &self.save_failure {
            let label = match failure.intent {
                SaveIntent::Save => "保存できませんでした",
                SaveIntent::SaveAs => "別名保存できませんでした",
            };
            header = header.child(div().text_color(palette.warning).child(label));
        }
        if !matches!(self.document.external_state(), ExternalState::Current) {
            header = header.child(
                div()
                    .text_color(palette.warning)
                    .child("外部ファイルに問題があります"),
            );
        }
        let header = header.child(div().flex_1());
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(4.))
            .bg(palette.surface)
            .child(self.action_button(UndoEdit, true, window, cx))
            .child(self.action_button(RedoEdit, true, window, cx))
            .child(self.action_button(AddSlide, false, window, cx))
            .child(self.action_button(ShowInsertMenu, false, window, cx))
            .child(self.action_button(ShowViewMenu, false, window, cx))
            .child(div().flex_1())
            .child(self.action_button(PresentCurrent, false, window, cx))
            .child(
                button(
                    cx,
                    "発表メニュー",
                    self.availability(window, cx).view,
                    |this, _, window, cx| this.show_popup(Popup::Present, window, cx),
                )
                .w_auto()
                .flex_shrink_0(),
            );
        let mut notices = div()
            .w_full()
            .min_w_0()
            .whitespace_normal()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .px(px(12.))
            .bg(palette.surface);
        if let Some(failure) = &self.save_failure {
            notices = notices.child(
                div()
                    .min_w_0()
                    .w_full()
                    .text_color(palette.warning)
                    .child(failure.message.clone()),
            );
        }
        if !matches!(self.document.external_state(), ExternalState::Current) {
            let reason = match self.document.external_state() {
                ExternalState::Conflict => "外部変更と未保存の編集が競合しています。",
                ExternalState::Invalid(_) => {
                    "外部ファイルが無効です。最後の正常な内容を表示しています。"
                }
                ExternalState::Unavailable {
                    kind: std::io::ErrorKind::NotFound,
                    ..
                } => "外部ファイルが削除されています。別名保存で編集を退避できます。",
                ExternalState::Unavailable { .. } => {
                    "外部ファイルを読み込めません。アクセス権などを確認してください。"
                }
                ExternalState::Current => unreachable!(),
            };
            notices = notices
                .child(div().text_color(palette.warning).child(reason))
                .child(
                    button(
                        cx,
                        "再読み込み",
                        self.document.path().is_some(),
                        |this, _, window, cx| this.reload_external_change(window, cx),
                    )
                    .w_auto()
                    .flex_shrink_0(),
                )
                .child(self.action_button(SaveAs, false, window, cx));
        }

        let side = self.render_slide_list(model.clone(), window, cx);
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
                .w_full()
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
                .w_full()
                .p_3()
                .text_color(self.palette.warning)
                .child("外部ファイルを読み込めません")
                .child(error.clone())
                .into_any_element()
        } else if model.is_none() {
            div()
                .p_3()
                .child("文書の診断を確認してください。編集はできません。読み取り専用です。")
                .into_any_element()
        } else {
            self.render_inspector(model.as_deref(), window, cx)
        };
        let mut footer = div()
            .id("editor-status")
            .max_h(px(160.))
            .flex_shrink_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .bg(palette.surface)
            .text_size(px(12.))
            .text_color(palette.secondary)
            .child(format!(
                "スライド {} / {} · {}% · {}",
                self.current_slide + 1,
                model.as_ref().map_or(0, |model| model.slides.len()),
                (self.view.scale() * 100.).round(),
                if self.view.zoom == ZoomMode::Fit {
                    "全体に合わせる"
                } else {
                    "100%表示"
                }
            ))
            .child(self.status.clone());
        if let Some(reason) = self.availability(window, cx).present_reason {
            footer = footer.child(div().text_color(palette.warning).child(reason));
        }
        let diagnostics = self.document.diagnostics().len()
            + self.asset_diagnostics.len()
            + self.layout_diagnostics.len()
            + match self.document.external_state() {
                ExternalState::Invalid(items) => items.len(),
                _ => 0,
            };
        footer = footer.child(button(
            cx,
            format!("診断 {diagnostics}件"),
            true,
            |this, _, window, cx| this.show_popup(Popup::Diagnostics, window, cx),
        ));
        if !matches!(self.document.external_state(), ExternalState::Current) {
            footer = footer.child(
                div()
                    .text_color(self.palette.warning)
                    .child("表示中の内容は現在の外部ファイルと一致していません。"),
            );
        }
        if let ExternalState::Unavailable { message: error, .. } = self.document.external_state() {
            footer = footer.child(
                div()
                    .text_color(self.palette.warning)
                    .child(format!("外部ファイルを読み込めません: {error}")),
            );
        }
        let (navigator_width, inspector_width) =
            self.view.widths(f32::from(window.viewport_size().width));
        let weak = cx.entity().downgrade();
        let viewport = div()
            .id("canvas-viewport")
            .track_focus(&self.canvas_focus)
            .role(gpui::Role::Group)
            .aria_label("スライドキャンバス")
            .focus(|style| style.border_2().border_color(palette.focus))
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_x_scroll()
            .overflow_y_scroll()
            .child(
                gpui::canvas(
                    move |bounds, _, cx| {
                        cx.defer(move |cx| {
                            weak.update(cx, |this, cx| {
                                if this.view.canvas_area != bounds.size {
                                    this.view.canvas_area = bounds.size;
                                    cx.notify();
                                }
                            })
                            .ok();
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(model.is_some(), |element| {
                        element
                            .w(px(1280. * self.view.scale()))
                            .h(px(720. * self.view.scale()))
                    })
                    .min_w_full()
                    .min_h_full()
                    .child(canvas),
            );
        let mut workspace = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .gap(px(0.))
            .p(px(8.));
        if self.view.navigator.visible {
            workspace = workspace
                .child(
                    div()
                        .w(px(navigator_width))
                        .h_full()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .flex_shrink_0()
                        .min_w_0()
                        .overflow_hidden()
                        .child(side),
                )
                .child(self.render_pane_resizer(Pane::Navigator, navigator_width, cx));
        }
        workspace = workspace.child(viewport);
        if self.view.inspector.visible {
            workspace = workspace
                .child(self.render_pane_resizer(Pane::Inspector, inspector_width, cx))
                .child(
                    div()
                        .w(px(inspector_width))
                        .h_full()
                        .flex_shrink_0()
                        .min_w_0()
                        .child(inspector),
                );
        }
        let popup = self.view.popup.clone().map(|popup| {
            let (left, top) = match popup {
                Popup::Slide(position) => (
                    f32::from(position.x).min(f32::from(window.viewport_size().width) - 260.),
                    f32::from(position.y).min(f32::from(window.viewport_size().height) - 160.),
                ),
                _ => (240., 88.),
            };
            let mut panel = div()
                .id("toolbar-popup")
                .role(gpui::Role::Group)
                .aria_label(match popup {
                    Popup::Insert(_) => "挿入",
                    Popup::View => "表示",
                    Popup::Diagnostics => "診断",
                    Popup::Present => "発表",
                    Popup::Slide(_) => "スライド",
                })
                .absolute()
                .top(px(top))
                .left(px(left))
                .w(px(260.))
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .bg(palette.surface)
                .border_1()
                .border_color(palette.border);
            match popup {
                Popup::Insert(state) => {
                    panel = panel
                        .w(px(360.))
                        .max_h(px(
                            (f32::from(window.viewport_size().height) - top - 16.).max(80.)
                        ))
                        .child(self.render_insert_menu(state, window, cx));
                }
                Popup::Slide(_) => {
                    panel = panel
                        .child(self.action_button(AddSlide, false, window, cx))
                        .child(self.action_button(DeleteSlide, false, window, cx))
                        .child(self.action_button(MoveSlideEarlier, false, window, cx))
                        .child(self.action_button(MoveSlideLater, false, window, cx));
                }
                Popup::Present => {
                    panel = panel
                        .child(self.action_button(PresentCurrent, false, window, cx))
                        .child(self.action_button(PresentFirst, false, window, cx));
                }
                Popup::Diagnostics => {
                    panel = panel
                        .w(px(540.))
                        .max_h(px(
                            (f32::from(window.viewport_size().height) - top - 16.).max(80.)
                        ))
                        .overflow_y_scroll();
                    panel = panel.child(diagnostics_view(
                        self.document.diagnostics(),
                        self.source_selection.as_ref(),
                        cx,
                    ));
                    if let ExternalState::Invalid(items) = self.document.external_state() {
                        panel = panel.child(diagnostics_view(
                            items,
                            self.source_selection.as_ref(),
                            cx,
                        ));
                    }
                    for issue in &self.asset_diagnostics {
                        let issue = issue.clone();
                        panel = panel.child(button(
                            cx,
                            format!(
                                "画像 · Slide {} · {} · {}",
                                issue.slide_index + 1,
                                issue.path,
                                issue.problem
                            ),
                            true,
                            move |this, _, window, cx| {
                                this.navigate_to_asset(issue.clone(), window, cx)
                            },
                        ));
                    }
                    for issue in self.layout_diagnostics.clone() {
                        let label = format!(
                            "Layout · Slide {} · Element {} · {}",
                            issue.slide_index + 1,
                            issue.element_index + 1,
                            issue.message
                        );
                        panel = panel.child(button(cx, label, true, move |this, _, window, cx| {
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
                            this.view.popup = None;
                            this.current_slide = issue.slide_index;
                            if kind == ElementKind::Columns {
                                this.select_columns(
                                    issue.slide_index,
                                    issue.element_index,
                                    window,
                                    cx,
                                );
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
                }
                Popup::View => {
                    panel = panel
                        .child(self.action_button(ToggleNavigator, false, window, cx))
                        .child(self.action_button(ToggleInspector, false, window, cx))
                        .child(self.action_button(FitCanvas, false, window, cx))
                        .child(self.action_button(ActualSize, false, window, cx));
                }
            }
            panel.into_any_element()
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .key_context("Editor")
            .track_focus(&self.root_focus)
            .on_mouse_move(cx.listener(|this, event, _, cx| {
                this.update_column_resize(event, cx);
                this.update_slide_drag_pointer(event.position);
                this.update_pane_resize(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.view.resize = None;
                    this.finish_column_resize(window, cx);
                    if this.drop_destination.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.view.resize = None;
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
            .on_action(cx.listener(|this, action: &OpenFile, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &AddSlide, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &DeleteSlide, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &MoveSlideEarlier, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &MoveSlideLater, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &InsertElement, window, cx| {
                this.insert_element(action.request.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, action: &ShowInsertMenu, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ShowViewMenu, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ToggleNavigator, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ToggleInspector, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &NarrowNavigator, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &WidenNavigator, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &NarrowInspector, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &WidenInspector, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &FitCanvas, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ActualSize, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &PresentCurrent, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &CloseFile, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ShowDiagnostics, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &PresentFirst, window, cx| {
                this.execute_action(action, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusNext, window, cx| {
                if !this.cycle_insert_focus(false, window, cx) {
                    window.focus_next(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusPrevious, window, cx| {
                if !this.cycle_insert_focus(true, window, cx) {
                    window.focus_prev(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ClosePopup, window, cx| {
                if this.view.popup.take().is_some() {
                    this.restore_popup_focus(window, cx);
                    cx.notify();
                }
            }))
            .relative()
            .text_color(palette.text)
            .bg(palette.background)
            .child(header)
            .child(toolbar)
            .child(notices)
            .child(workspace)
            .child(footer)
            .children(popup)
    }

    fn render_element_outline(
        &mut self,
        model: &PresentationModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let rows = outline_rows(model, self.current_slide);
        let selected = selected_source(self.target);
        let exposure = (self.current_slide, selected);
        if self.outline_exposure != Some(exposure) {
            if let Some(index) = rows.iter().position(|row| Some(row.source) == selected) {
                self.outline_scroll.scroll_to_item(index);
            } else if self
                .outline_exposure
                .is_some_and(|old| old.0 != self.current_slide)
            {
                self.outline_scroll.set_offset(gpui::point(px(0.), px(0.)));
            }
            self.inspector_scroll
                .set_offset(gpui::point(px(0.), px(0.)));
            self.outline_exposure = Some(exposure);
        }
        while self.outline_focus.len() < rows.len() {
            let index = self.outline_focus.len();
            let focus = cx.focus_handle().tab_stop(true);
            let subscription = cx.on_focus(&focus, window, move |this, window, cx| {
                this.outline_scroll.scroll_to_item(index);
                window.on_next_frame(|window, _| window.refresh());
                cx.notify();
            });
            self.outline_focus.push((focus, subscription));
        }
        self.outline_focus.truncate(rows.len());
        let mut outline = div()
            .id("element-outline")
            .role(gpui::Role::ListBox)
            .aria_label("スライドの構造一覧")
            .aria_description("上下キーで項目を選択、Tabで設定へ移動")
            .flex()
            .flex_col()
            .h(px(180.))
            .min_h_0()
            .flex_shrink_0()
            .overflow_y_scroll()
            .track_scroll(&self.outline_scroll)
            .gap_1();
        let tab_index = rows
            .iter()
            .position(|row| Some(row.source) == selected)
            .unwrap_or(0);
        for (index, row) in rows.into_iter().enumerate() {
            self.outline_focus[index].0 = self.outline_focus[index]
                .0
                .clone()
                .tab_stop(index == tab_index);
            outline = outline.child(self.render_outline_row(
                row.source,
                row.label,
                index,
                index == tab_index,
                cx,
            ));
        }
        outline.into_any_element()
    }

    fn render_outline_row(
        &self,
        source: CanvasSource,
        label: String,
        row_index: usize,
        tab_stop: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let selected = selected_source(self.target) == Some(source);
        styled_button(
            cx,
            format!("{label}{}", if selected { " ✓" } else { "" }),
            true,
            ButtonChrome::Inspector,
            move |this, _, window, cx| {
                this.select_canvas_source(source, window, cx);
                if selected_source(this.target) == Some(source) {
                    window.focus(&this.outline_focus[row_index].0, cx);
                }
            },
        )
        .id(format!("outline-{}-{row_index}", self.current_slide))
        .accessibility_id(format!("outline-{}-{row_index}", self.current_slide))
        .flex_shrink_0()
        .tab_stop(tab_stop)
        .role(gpui::Role::ListBoxOption)
        .aria_label(match source {
            CanvasSource::NestedElement { columns, .. } => format!("2列 {} · {label}", columns + 1),
            CanvasSource::Element { .. } => label,
        })
        .aria_selected(selected)
        .track_focus(&self.outline_focus[row_index].0)
        .when(
            matches!(source, CanvasSource::NestedElement { .. }),
            |row| row.ml_3(),
        )
        .on_key_down(
            cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                let delta = match event.keystroke.key.as_str() {
                    "up" => -1,
                    "down" => 1,
                    "enter" | "space" => 0,
                    _ => return,
                };
                let Some(model) = this.preview_model(cx) else {
                    return;
                };
                let rows = outline_rows(&model, this.current_slide);
                let next = (row_index as isize + delta)
                    .clamp(0, rows.len().saturating_sub(1) as isize)
                    as usize;
                if let Some(row) = rows.get(next) {
                    this.select_canvas_source(row.source, window, cx);
                    if selected_source(this.target) == Some(row.source) {
                        window.focus(&this.outline_focus[next].0, cx);
                    }
                }
                cx.stop_propagation();
                window.prevent_default();
            }),
        )
        .into_any_element()
    }

    fn render_inspector(
        &mut self,
        model: Option<&PresentationModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let palette = self.palette;
        let mut inspector = div()
            .id("inspector")
            .flex()
            .flex_col()
            .w_full()
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .gap(px(4.))
            .p_3()
            .bg(palette.surface);
        inspector = inspector.child(div().font_weight(gpui::FontWeight::BOLD).child("Inspector"));
        if self.external_edit_blocked() {
            return inspector
                .child("外部ファイルの問題を解決するまで編集できません。")
                .child("保持中の入力")
                .child(div().w_full().child(self.editor.read(cx).value()))
                .into_any_element();
        }
        if let Some(model) = model {
            let outline = self.render_element_outline(model, window, cx);
            let mut settings = div()
                .id("inspector-settings")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.inspector_scroll)
                .gap(px(4.));
            settings = settings.child(inspector_button(
                window,
                &self.inspector_scroll,
                cx,
                "Presentation title",
                true,
                ButtonChrome::Inspector,
                |this, _, window, cx| this.select_title(window, cx),
            ));
            let element = selected_element(model, self.target);
            settings = settings.child(inspector_section_label(
                element.map_or("Presentation のタイトル", outline_kind_label),
                palette,
            ));
            if !matches!(self.target, EditTarget::Columns { .. }) {
                if let Err(DocumentError::UnsafeValue(reason)) =
                    validate_editor_value(self.target, &self.editor.read(cx).value())
                {
                    inspector =
                        inspector.child(div().text_color(self.palette.warning).child(reason));
                }
                settings = settings.child(inspector_button(
                    window,
                    &self.inspector_scroll,
                    cx,
                    "入力を取り消す",
                    self.has_uncommitted_draft(cx),
                    ButtonChrome::Inspector,
                    |this, _, window, cx| this.cancel_input(window, cx),
                ));
            }
            if let Some(element) = element {
                if self.canvas_editing && supports_canvas_text_editor(element, self.target) {
                    settings = settings
                        .child(
                            div()
                                .pt_2()
                                .text_size(px(12.0))
                                .child("キャンバス上で編集中"),
                        )
                        .child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "全文をInspectorで編集",
                            true,
                            ButtonChrome::Inspector,
                            |this, _, window, cx| {
                                this.canvas_editing = false;
                                this.ime_geometry.refresh(window, true);
                                let focus = this.editor.read(cx).focus_handle();
                                window.focus(&focus, cx);
                                cx.notify();
                            },
                        ));
                } else if !matches!(element, Element::Columns { .. }) {
                    settings = settings
                        .child(inspector_section_label(self.editor_label(), palette))
                        .child(if self.editor.read(cx).is_multiline() {
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.secondary)
                                .child("Enter で改行できます")
                        } else {
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.secondary)
                                .child("1 行で編集します")
                        })
                        .child(inspector_input(
                            window,
                            &self.inspector_scroll,
                            cx,
                            self.editor.clone(),
                        ))
                        .child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "適用",
                            true,
                            ButtonChrome::InspectorPrimary,
                            |this, _, window, cx| {
                                match this.commit_draft(window, cx) {
                                    Ok(()) => window.focus(&this.root_focus, cx),
                                    Err(error) => this.status = error,
                                }
                                cx.notify();
                            },
                        ));
                }
                if let EditTarget::ColumnWidth { left, .. } = self.target {
                    settings = settings
                        .child(div().pt_2().child(if left {
                            "左列幅（1〜99）"
                        } else {
                            "右列幅（1〜99）"
                        }))
                        .child(inspector_input(
                            window,
                            &self.inspector_scroll,
                            cx,
                            self.editor.clone(),
                        ))
                        .child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "幅を適用",
                            true,
                            ButtonChrome::InspectorPrimary,
                            |this, _, window, cx| {
                                match this.commit_draft(window, cx) {
                                    Ok(()) => window.focus(&this.root_focus, cx),
                                    Err(error) => this.status = error,
                                }
                                cx.notify();
                            },
                        ));
                }
                match element {
                    Element::Heading(_) | Element::Text(_) => {}
                    Element::Bullets(items) => {
                        settings = settings.child(div().pt_2().child("箇条書き項目"));
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
                            settings = settings.child(inspector_button(
                                window,
                                &self.inspector_scroll,
                                cx,
                                format!(
                                    "項目 {}{}",
                                    item_index + 1,
                                    if selected { " ✓" } else { "" }
                                ),
                                true,
                                ButtonChrome::Inspector,
                                move |this, _, window, cx| {
                                    this.select_current_field(
                                        ElementField::Bullet(item_index),
                                        window,
                                        cx,
                                    )
                                },
                            ));
                            if selected && item_actions_available {
                                settings = settings
                                    .child(inspector_button_pair(
                                        window,
                                        &self.inspector_scroll,
                                        cx,
                                        "項目を上へ",
                                        item_index > 0,
                                        |this, _, window, cx| {
                                            this.move_bullet_selection(-1, window, cx)
                                        },
                                        "項目を下へ",
                                        item_index + 1 < items.len(),
                                        |this, _, window, cx| {
                                            this.move_bullet_selection(1, window, cx)
                                        },
                                    ))
                                    .child(inspector_button(
                                        window,
                                        &self.inspector_scroll,
                                        cx,
                                        "項目を削除",
                                        true,
                                        ButtonChrome::Inspector,
                                        |this, _, window, cx| {
                                            this.remove_bullet_selection(window, cx)
                                        },
                                    ));
                            }
                        }
                        if item_actions_available {
                            settings = settings.child(inspector_button(
                                window,
                                &self.inspector_scroll,
                                cx,
                                "項目を追加",
                                true,
                                ButtonChrome::Inspector,
                                |this, _, window, cx| this.add_bullet_selection(window, cx),
                            ));
                        }
                    }
                    Element::Code { language, .. } => {
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "コード全文",
                            true,
                            ButtonChrome::Inspector,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::Text, window, cx)
                            },
                        ));
                        settings = settings.child(format!(
                            "現在の言語: {}",
                            language.as_deref().unwrap_or("なし")
                        ));
                        settings = settings.child(
                            div()
                                .text_size(px(11.))
                                .child("色分けの候補。その他の言語も入力できます。"),
                        );
                        for preset in ["rust", "json", "yaml", "toml", ""] {
                            settings = settings.child(inspector_button(
                                window,
                                &self.inspector_scroll,
                                cx,
                                format!(
                                    "言語を{}に変更",
                                    if preset.is_empty() { "なし" } else { preset }
                                ),
                                true,
                                ButtonChrome::Inspector,
                                move |this, _, window, cx| {
                                    this.apply_code_language(preset, window, cx)
                                },
                            ));
                        }
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "コード言語",
                            true,
                            ButtonChrome::Inspector,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::CodeLanguage, window, cx)
                            },
                        ));
                    }
                    Element::Image { .. } => {
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "画像パス",
                            true,
                            ButtonChrome::Inspector,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::ImagePath, window, cx)
                            },
                        ));
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "Caption",
                            true,
                            ButtonChrome::Inspector,
                            |this, _, window, cx| {
                                this.select_current_field(ElementField::Caption, window, cx)
                            },
                        ));
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "画像を再読み込み",
                            true,
                            ButtonChrome::Inspector,
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
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "Element を削除",
                            true,
                            ButtonChrome::Inspector,
                            Self::delete_element,
                        ));
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "左列幅を数値入力",
                            true,
                            ButtonChrome::Inspector,
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
                        settings = settings.child(inspector_button(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "右列幅を数値入力",
                            true,
                            ButtonChrome::Inspector,
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
                        settings = settings.child(inspector_button_pair(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "左列を +5%",
                            *left_width <= 94,
                            |this, _, _, cx| this.resize_columns(5, cx),
                            "左列を -5%",
                            *left_width >= 6,
                            |this, _, _, cx| this.resize_columns(-5, cx),
                        ));
                        for (column, label) in [(0, "左列"), (1, "右列")] {
                            settings = settings.child(
                                div()
                                    .pt_2()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(label),
                            );
                            let parent = match self.target {
                                EditTarget::Columns { index, .. }
                                | EditTarget::ColumnWidth { index, .. } => InsertParent::Column {
                                    columns: index,
                                    side: if column == 0 {
                                        ColumnSide::Left
                                    } else {
                                        ColumnSide::Right
                                    },
                                },
                                _ => unreachable!(),
                            };
                            settings = settings.child(inspector_button(
                                window,
                                &self.inspector_scroll,
                                cx,
                                format!("{label}に追加…"),
                                !self.external_edit_blocked(),
                                ButtonChrome::Inspector,
                                move |this, _, window, cx| {
                                    this.open_insert_menu(parent, window, cx);
                                },
                            ));
                        }
                    }
                }
                if let EditTarget::NestedElement { slide, columns, .. } = self.target {
                    settings = settings.child(inspector_button(
                        window,
                        &self.inspector_scroll,
                        cx,
                        "2列の内容へ戻る",
                        true,
                        ButtonChrome::Inspector,
                        move |this, _, window, cx| this.select_columns(slide, columns, window, cx),
                    ));
                    settings = settings.child(inspector_button(
                        window,
                        &self.inspector_scroll,
                        cx,
                        "Slide 直下へ移動",
                        true,
                        ButtonChrome::Inspector,
                        |this, _, window, cx| this.move_nested_element_to_slide(window, cx),
                    ));
                }
                if let EditTarget::Element { slide, .. } = self.target {
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
                                settings = settings.child(inspector_button(
                                    window,
                                    &self.inspector_scroll,
                                    cx,
                                    format!("2列 {} · {label}へ移動", columns + 1),
                                    true,
                                    ButtonChrome::Inspector,
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
                        settings = settings.child(inspector_button_pair(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "Element ↑",
                            index > 0,
                            |this, _, window, cx| this.move_element_selection(-1, window, cx),
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
                        settings = settings.child(inspector_button_pair(
                            window,
                            &self.inspector_scroll,
                            cx,
                            "列内 Element ↑",
                            index > 0,
                            |this, _, window, cx| {
                                this.move_nested_element_selection(-1, window, cx)
                            },
                            "列内 Element ↓",
                            index + 1 < count,
                            |this, _, window, cx| this.move_nested_element_selection(1, window, cx),
                        ));
                    }
                }
                if !matches!(element, Element::Columns { .. }) {
                    settings = settings.child(inspector_button(
                        window,
                        &self.inspector_scroll,
                        cx,
                        "Element を削除",
                        true,
                        ButtonChrome::Inspector,
                        Self::delete_element,
                    ));
                }
            } else if let EditTarget::Title = self.target {
                settings = settings
                    .child(inspector_section_label(self.editor_label(), palette))
                    .child(
                        inspector_input(window, &self.inspector_scroll, cx, self.editor.clone())
                            .h(px(38.)),
                    )
                    .child(inspector_button(
                        window,
                        &self.inspector_scroll,
                        cx,
                        "適用",
                        true,
                        ButtonChrome::InspectorPrimary,
                        |this, _, window, cx| {
                            if let Err(error) = this.commit_draft(window, cx) {
                                this.status = error;
                            } else {
                                window.focus(&this.root_focus, cx);
                            }
                            cx.notify();
                        },
                    ));
            }
            if !matches!(self.target, EditTarget::Columns { .. }) {
                settings = settings.child(inspector_button(
                    window,
                    &self.inspector_scroll,
                    cx,
                    "要素を追加…",
                    !self.external_edit_blocked(),
                    ButtonChrome::InspectorGhost,
                    |this, _, window, cx| {
                        this.open_insert_menu(InsertParent::Slide, window, cx);
                    },
                ));
            }
            return inspector
                .child(inspector_section_label("スライドの構造", palette))
                .child(outline)
                .child(settings)
                .into_any_element();
        }
        inspector.into_any_element()
    }

    fn apply_code_language(&mut self, language: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.external_edit_blocked() {
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.target = match self.target {
            EditTarget::Element { slide, index, .. } => EditTarget::Element {
                slide,
                index,
                field: ElementField::CodeLanguage,
            },
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                field: ElementField::CodeLanguage,
            },
            _ => return,
        };
        self.canvas_editing = false;
        self.editor.update(cx, |input, cx| {
            input.set_multiline(false, cx);
            input.set_value(language, cx);
        });
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
        }
        cx.notify();
    }

    fn editor_label(&self) -> &'static str {
        match self.target {
            EditTarget::Title => "Presentation のタイトル",
            EditTarget::ColumnWidth { left: true, .. } => "左列幅、1〜99の整数",
            EditTarget::ColumnWidth { left: false, .. } => "右列幅、1〜99の整数",
            EditTarget::Columns { .. } => "2列の設定",
            EditTarget::Element { field, .. } | EditTarget::NestedElement { field, .. } => {
                match field {
                    ElementField::CodeLanguage => "コード言語、任意の文字列",
                    ElementField::ImagePath => "画像パス、文書からの相対パス",
                    ElementField::Caption => "画像のCaption",
                    ElementField::Bullet(_) => "箇条書きの項目",
                    ElementField::Text => self
                        .document
                        .model()
                        .and_then(|model| selected_element(model, self.target))
                        .map_or("本文", |element| match element {
                            Element::Heading(_) => "見出し",
                            Element::Bullets(_) => "箇条書き全文、1行に1項目",
                            Element::Code { .. } => "コード全文",
                            _ => "本文",
                        }),
                }
            }
        }
    }

    fn select_current_field(
        &mut self,
        field: ElementField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.target {
            EditTarget::Element { slide, index, .. } => {
                self.select_element(slide, index, field, false, window, cx)
            }
            EditTarget::NestedElement {
                slide,
                columns,
                column,
                index,
                ..
            } => {
                self.select_nested_element(slide, columns, column, index, field, false, window, cx)
            }
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
        let scale = if editing {
            self.view.scale()
        } else {
            let viewport = window.viewport_size();
            (f32::from(viewport.width) / 1280.).min(f32::from(viewport.height) / 720.)
        };
        let images = if editing {
            &self.asset_images
        } else {
            self.presentation
                .as_ref()
                .map(|presentation| &presentation.render_images)
                .unwrap_or(&self.asset_images)
        };
        let canvas = self.render_slide_body(
            model.slides.get(slide_index),
            slide_index,
            scale,
            if editing {
                SlideInteraction::Editor
            } else {
                SlideInteraction::ReadOnly
            },
            images,
            window,
            cx,
        );
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
            .when(editing && !self.external_edit_blocked(), |canvas| {
                canvas.on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if !cx.has_active_drag() && this.column_resize_drag.is_none() {
                            this.select_title(window, cx);
                        }
                    }),
                )
            })
            .child(canvas)
            .into_any_element()
    }

    fn render_slide_body(
        &self,
        slide: Option<&psycho::Slide>,
        slide_index: usize,
        scale: f32,
        interaction: SlideInteraction,
        images: &HashMap<String, Arc<gpui::RenderImage>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 1280.0 * scale;
        let height = 720.0 * scale;
        let editing = interaction == SlideInteraction::Editor;
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
            .flex_shrink_0()
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
        canvas.into_any_element()
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
        let selected = editing && selected;
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
                let preview_width = if editing {
                    self.column_resize_drag
                        .filter(|drag| drag.slide == parent_slide && drag.index == parent_columns)
                        .map_or(*left_width, |drag| drag.preview_width)
                } else {
                    *left_width
                };
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
        let label = self.editor_label();
        self.editor
            .update(cx, |input, cx| input.set_accessible_label(label, cx));
        self.editor.read(cx).record_acceptance_state(window);
        editor_validation::record(self, window, cx);
        if self.presentation.is_none() {
            self.layout_diagnostics = self
                .preview_model(cx)
                .as_ref()
                .map(|model| collect_layout_diagnostics(model, window))
                .unwrap_or_default();
        }
        let available = self.availability(window, cx);
        let input = self.editor.read(cx);
        let input_focused = available.edit && input.has_focus(window);
        let can_copy = input_focused && input.selected_text().is_some();
        let undo_label = if input_focused {
            "元に戻す: 文字編集".to_owned()
        } else {
            self.document.undo_description().map_or_else(
                || "元に戻す".to_owned(),
                |description| format!("元に戻す: {description}"),
            )
        };
        let redo_label = if input_focused {
            "やり直す: 文字編集".to_owned()
        } else {
            self.document.redo_description().map_or_else(
                || "やり直す".to_owned(),
                |description| format!("やり直す: {description}"),
            )
        };
        let undo_item = if input_focused {
            MenuItem::action(undo_label, input::Undo).disabled(!available.undo)
        } else {
            MenuItem::action(undo_label, UndoEdit).disabled(!available.undo)
        };
        let redo_item = if input_focused {
            MenuItem::action(redo_label, input::Redo).disabled(!available.redo)
        } else {
            MenuItem::action(redo_label, RedoEdit).disabled(!available.redo)
        };
        cx.set_menus([
            Menu::new("psycho").items([MenuItem::action("psycho を隠す", HideApp)]),
            Menu::new("ファイル").items([
                action_menu(OpenFile, available),
                action_menu(CloseFile, available),
                action_menu(SaveFile, available),
                action_menu(SaveAs, available),
            ]),
            Menu::new("編集").items([
                undo_item,
                redo_item,
                MenuItem::action("切り取り", input::Cut).disabled(!can_copy),
                MenuItem::action("コピー", input::Copy).disabled(!can_copy),
                MenuItem::action("貼り付け", input::Paste).disabled(!input_focused),
                MenuItem::action("すべてを選択", input::SelectAll).disabled(!input_focused),
            ]),
            self.native_insert_menu(window, cx),
            Menu::new("スライド").items([
                action_menu(AddSlide, available),
                action_menu(DeleteSlide, available),
                action_menu(MoveSlideEarlier, available),
                action_menu(MoveSlideLater, available),
            ]),
            Menu::new("表示").items([
                MenuItem::action(
                    if self.view.navigator.visible {
                        "Navigatorを隠す"
                    } else {
                        "Navigatorを表示"
                    },
                    ToggleNavigator,
                )
                .disabled(!available.view),
                MenuItem::action(
                    if self.view.inspector.visible {
                        "Inspectorを隠す"
                    } else {
                        "Inspectorを表示"
                    },
                    ToggleInspector,
                )
                .disabled(!available.view),
                action_menu(NarrowNavigator, available),
                action_menu(WidenNavigator, available),
                action_menu(NarrowInspector, available),
                action_menu(WidenInspector, available),
                action_menu(FitCanvas, available),
                action_menu(ActualSize, available),
                action_menu(ShowDiagnostics, available),
            ]),
            Menu::new("プレゼンテーション").items([
                MenuItem::action("現在からプレゼンテーション", PresentCurrent)
                    .disabled(!available.present),
                action_menu(PresentFirst, available),
            ]),
        ]);
        #[cfg(target_os = "macos")]
        editor_insert::decorate_native_menus();
        if let Some(presentation) = self.presentation.as_ref() {
            let model = presentation.session.model().clone();
            let slide_index = presentation.session.current_index();
            div()
                .size_full()
                .key_context("Presentation")
                .track_focus(&self.root_focus)
                .on_mouse_move(cx.listener(|this, _, window, cx| this.pointer_moved(window, cx)))
                .on_mouse_up(MouseButton::Left, cx.listener(|_, _, _, cx| cx.notify()))
                .on_action(cx.listener(Self::next_slide))
                .on_action(cx.listener(Self::previous_slide))
                .on_action(cx.listener(Self::exit_presentation))
                .child(self.render_canvas(&model, slide_index, false, window, cx))
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
        } if value.is_empty() || value.starts_with('/') || Path::new(value).is_absolute() => {
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

fn selected_source(target: EditTarget) -> Option<CanvasSource> {
    match target {
        EditTarget::Title => None,
        EditTarget::Element { slide, index, .. }
        | EditTarget::Columns { slide, index }
        | EditTarget::ColumnWidth { slide, index, .. } => {
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
    }
}

fn selected_element(model: &PresentationModel, target: EditTarget) -> Option<&Element> {
    match selected_source(target)? {
        CanvasSource::Element { slide, index } => model.slides.get(slide)?.elements.get(index),
        CanvasSource::NestedElement {
            slide,
            columns,
            column,
            index,
        } => nested_element(model, slide, columns, column, index),
    }
}

fn outline_rows(model: &PresentationModel, slide: usize) -> Vec<OutlineRow> {
    let Some(current) = model.slides.get(slide) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for (index, element) in current.elements.iter().enumerate() {
        rows.push(OutlineRow {
            source: CanvasSource::Element { slide, index },
            label: format!("Element {} · {}", index + 1, outline_kind_label(element)),
        });
        if let Element::Columns { left, right, .. } = element {
            for (column, label, elements) in [(0, "左列", left), (1, "右列", right)] {
                for (nested_index, nested) in elements.iter().enumerate() {
                    rows.push(OutlineRow {
                        source: CanvasSource::NestedElement {
                            slide,
                            columns: index,
                            column,
                            index: nested_index,
                        },
                        label: format!(
                            "{label} · {} {}",
                            outline_kind_label(nested),
                            nested_index + 1
                        ),
                    });
                }
            }
        }
    }
    rows
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
    let palette = *cx.global::<EditorPalette>();
    let mut list = div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .min_w_0()
        .max_w(px(680.0))
        .whitespace_normal()
        .p_6()
        .bg(palette.surface)
        .text_color(palette.text);
    if diagnostics.is_empty() {
        list = list.child("構文・スキーマの診断はありません。");
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
        let row = div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_1()
            .border_color(palette.border)
            .bg(if selected_source == Some(diagnostic) {
                palette.selection
            } else {
                palette.surface
            })
            .text_color(palette.warning)
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
        let accessible_target = diagnostic.clone();
        let weak = cx.entity().downgrade();
        let label = format!(
            "{kind} · {file_name} · {}行 {}列 · {}",
            diagnostic.line, diagnostic.column, diagnostic.message
        );
        let row = div()
            .id(label.clone())
            .focusable()
            .tab_stop(true)
            .role(gpui::Role::Button)
            .aria_label(label)
            .focus(|style| style.border_2().border_color(palette.focus))
            .cursor_pointer()
            .child(row)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.navigate_to_diagnostic(target.clone(), window, cx)
            }))
            .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                weak.update(cx, |this, cx| {
                    this.navigate_to_diagnostic(accessible_target.clone(), window, cx)
                })
                .ok();
            });
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
                .border_color(palette.focus)
                .bg(palette.selection)
                .text_color(palette.text)
                .child(format!(
                    "読み取り専用のソース行 · {file_name}:{}:{}",
                    diagnostic.line, diagnostic.column
                ))
                .child(div().font_family("Menlo").text_size(px(13.0)).child(line))
                .child(
                    div()
                        .font_family("Menlo")
                        .text_size(px(13.0))
                        .text_color(palette.focus)
                        .child(format!(
                            "{}^",
                            " ".repeat(diagnostic.column.saturating_sub(1))
                        )),
                ),
        );
    }
    list
}

#[derive(Clone, Copy)]
enum ButtonChrome {
    Panel,
    Inspector,
    InspectorPrimary,
    InspectorGhost,
}

fn button(
    cx: &mut Context<PsychoApp>,
    label: impl Into<gpui::SharedString>,
    enabled: bool,
    handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    styled_button(cx, label, enabled, ButtonChrome::Panel, handler)
}

fn inspector_focus(
    window: &mut Window,
    scroll: &gpui::ScrollHandle,
    cx: &mut Context<PsychoApp>,
    key: &str,
    focus: Option<FocusHandle>,
) -> (FocusHandle, gpui::ScrollAnchor) {
    let anchor_scroll = scroll.clone();
    let control = window.use_keyed_state(format!("inspector-control-{key}"), cx, |window, cx| {
        let focus = focus.unwrap_or_else(|| cx.focus_handle());
        let anchor = gpui::ScrollAnchor::for_handle(anchor_scroll);
        let reveal = anchor.clone();
        let subscription = cx.on_focus(&focus, window, move |_, window, cx| {
            reveal.scroll_to(window, cx);
            window.on_next_frame(|window, _| window.refresh());
            cx.notify();
        });
        InspectorControl {
            focus,
            anchor,
            _subscription: subscription,
        }
    });
    let control = control.read(cx);
    (control.focus.clone(), control.anchor.clone())
}

fn inspector_input(
    window: &mut Window,
    scroll: &gpui::ScrollHandle,
    cx: &mut Context<PsychoApp>,
    editor: gpui::Entity<TextInputState>,
) -> gpui::Stateful<gpui::Div> {
    let focus = editor.read(cx).focus_handle();
    let (_, anchor) = inspector_focus(window, scroll, cx, "text-input", Some(focus));
    let palette = *cx.global::<EditorPalette>();
    div()
        .id("inspector-input")
        .anchor_scroll(Some(anchor))
        .w_full()
        .px_2()
        .py_1()
        .border_1()
        .border_color(palette.border)
        .bg(palette.input)
        .text_color(palette.text)
        .child(editor)
}

fn inspector_button(
    window: &mut Window,
    scroll: &gpui::ScrollHandle,
    cx: &mut Context<PsychoApp>,
    label: impl Into<gpui::SharedString>,
    enabled: bool,
    chrome: ButtonChrome,
    handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>) + 'static,
) -> impl IntoElement {
    let label = label.into();
    let (focus, anchor) = inspector_focus(window, scroll, cx, &label, None);
    div().w_full().flex_shrink_0().child(
        styled_button(cx, label, enabled, chrome, handler)
            .track_focus(&focus.tab_stop(enabled))
            .anchor_scroll(Some(anchor)),
    )
}

fn inspector_button_pair(
    window: &mut Window,
    scroll: &gpui::ScrollHandle,
    cx: &mut Context<PsychoApp>,
    left_label: impl Into<gpui::SharedString>,
    left_enabled: bool,
    left_handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>) + 'static,
    right_label: impl Into<gpui::SharedString>,
    right_enabled: bool,
    right_handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>)
    + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .w_full()
        .gap(px(4.))
        .child(div().flex_1().min_w_0().child(inspector_button(
            window,
            scroll,
            cx,
            left_label,
            left_enabled,
            ButtonChrome::Inspector,
            left_handler,
        )))
        .child(div().flex_1().min_w_0().child(inspector_button(
            window,
            scroll,
            cx,
            right_label,
            right_enabled,
            ButtonChrome::Inspector,
            right_handler,
        )))
}

fn inspector_section_label(
    text: impl Into<gpui::SharedString>,
    palette: EditorPalette,
) -> impl IntoElement {
    div()
        .pt(px(8.))
        .pb(px(4.))
        .text_size(px(12.))
        .text_color(palette.secondary)
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(text.into())
}

fn styled_button(
    cx: &mut Context<PsychoApp>,
    label: impl Into<gpui::SharedString>,
    enabled: bool,
    chrome: ButtonChrome,
    handler: impl Fn(&mut PsychoApp, &MouseUpEvent, &mut Window, &mut Context<PsychoApp>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let label = label.into();
    let palette = *cx.global::<EditorPalette>();
    let (background, border_color, border_width) = match chrome {
        ButtonChrome::Panel | ButtonChrome::Inspector => (palette.surface, palette.border, 1),
        ButtonChrome::InspectorPrimary => (palette.selection, palette.focus, 1),
        ButtonChrome::InspectorGhost => (palette.surface, palette.border, 0),
    };
    let handler = std::rc::Rc::new(handler);
    let click_handler = handler.clone();
    let weak = cx.entity().downgrade();
    let mut control = div()
        .id(label.clone())
        .accessibility_id(label.clone())
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .focusable()
        .tab_stop(enabled)
        .a11y_synthetic_children(move |builder| {
            if !enabled {
                builder.parent_node().set_disabled();
            }
        })
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(28.))
        .w_full()
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(13.))
        .bg(background)
        .text_color(if enabled {
            palette.text
        } else {
            palette.secondary
        })
        .focus(|style| style.border_2().border_color(palette.focus))
        .cursor_pointer()
        .child(label)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_click(cx.listener(move |this, event, window, cx| {
            if enabled {
                let event = match event {
                    gpui::ClickEvent::Mouse(mouse) => mouse.up.clone(),
                    _ => MouseUpEvent::default(),
                };
                click_handler(this, &event, window, cx);
            }
        }))
        .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
            if enabled {
                weak.update(cx, |this, cx| {
                    handler(this, &MouseUpEvent::default(), window, cx)
                })
                .ok();
            }
        });
    if border_width > 0 {
        control = control.border_1().border_color(border_color);
    }
    if matches!(
        chrome,
        ButtonChrome::Inspector | ButtonChrome::InspectorPrimary | ButtonChrome::InspectorGhost
    ) {
        control = control.text_size(px(12.));
    }
    control
}

fn action_menu<A: EditorAction>(action: A, available: EditorAvailability) -> MenuItem {
    let enabled = action.enabled(available);
    MenuItem::action(A::LABEL, action).disabled(!enabled)
}

struct EditorTooltip(gpui::SharedString);
impl Render for EditorTooltip {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = EditorPalette::for_appearance(window.appearance());
        div()
            .p(px(8.))
            .rounded_md()
            .text_size(px(12.))
            .bg(palette.surface)
            .text_color(palette.text)
            .border_1()
            .border_color(palette.border)
            .child(self.0.clone())
    }
}

fn main() {
    application()
        .with_assets(editor_view::IconAssets)
        .run(|cx: &mut App| {
            cx.on_action(|_: &HideApp, cx| cx.hide());
            cx.bind_keys([
                KeyBinding::new("cmd-h", HideApp, None),
                KeyBinding::new("cmd-o", OpenFile, Some("Editor")),
                KeyBinding::new("cmd-w", CloseFile, Some("Editor")),
                KeyBinding::new("tab", FocusNext, Some("Editor")),
                KeyBinding::new("shift-tab", FocusPrevious, Some("Editor")),
                KeyBinding::new("escape", ClosePopup, Some("Editor")),
                KeyBinding::new("cmd-alt-1", ToggleNavigator, Some("Editor")),
                KeyBinding::new("cmd-alt-2", ToggleInspector, Some("Editor")),
                KeyBinding::new("cmd-0", FitCanvas, Some("Editor")),
                KeyBinding::new("cmd-1", ActualSize, Some("Editor")),
                KeyBinding::new("cmd-alt-p", PresentCurrent, Some("Editor")),
                KeyBinding::new("cmd-s", SaveFile, Some("Editor")),
                KeyBinding::new("cmd-shift-s", SaveAs, Some("Editor")),
                KeyBinding::new("cmd-z", UndoEdit, Some("Editor")),
                KeyBinding::new("cmd-shift-z", RedoEdit, Some("Editor")),
                KeyBinding::new("delete", DeleteSelection, Some("Editor")),
                KeyBinding::new("backspace", DeleteSlide, Some("SlideNavigator")),
                KeyBinding::new("delete", DeleteSlide, Some("SlideNavigator")),
                KeyBinding::new("cmd-up", MoveSlideEarlier, Some("SlideNavigator")),
                KeyBinding::new("cmd-down", MoveSlideLater, Some("SlideNavigator")),
                KeyBinding::new("cmd-shift-n", AddSlide, Some("SlideNavigator")),
                KeyBinding::new("up", navigator::SelectPreviousSlide, Some("SlideNavigator")),
                KeyBinding::new("down", navigator::SelectNextSlide, Some("SlideNavigator")),
                KeyBinding::new("enter", navigator::FocusSlideCanvas, Some("SlideNavigator")),
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
                Some(path) => {
                    PresentationDocument::open(path).expect("open requested presentation")
                }
                None => PresentationDocument::from_source_with_asset_base(
                    DEMO,
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples"),
                )
                .expect("load built-in example"),
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
                        editor_validation::configure_appearance(window);
                        let editor = cx.new(|cx| TextInputState::new(cx, &title));
                        let view = cx.new(|cx| PsychoApp::new(cx, document, editor, window));
                        let weak_view = view.downgrade();
                        let weak_watch = view.downgrade();
                        window.on_window_should_close(cx, move |window, cx| {
                            weak_view
                                .update(cx, |this, cx| {
                                    if !matches!(this.document_flow, DocumentFlow::Idle) {
                                        return false;
                                    }
                                    if this.document.is_dirty() || this.has_uncommitted_draft(cx) {
                                        this.request_document_action(
                                            DocumentAction::Close,
                                            window,
                                            cx,
                                        );
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
