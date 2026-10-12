use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ColumnSide {
    Left,
    Right,
}
impl ColumnSide {
    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Left => "左列",
            Self::Right => "右列",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InsertParent {
    Slide,
    Column { columns: usize, side: ColumnSide },
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct InsertRequest {
    slide: usize,
    parent: InsertParent,
    kind: ElementKind,
    source: Arc<str>,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct InsertMenuState {
    slide: usize,
    parent: InsertParent,
    source: Arc<str>,
    parents: Vec<(InsertParent, String)>,
    focus: Vec<FocusHandle>,
}
#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub(super) struct InsertElement {
    pub(super) request: InsertRequest,
}

struct InsertOption {
    kind: ElementKind,
    label: &'static str,
    description: &'static str,
    icon: &'static str,
    svg: &'static [u8],
}
macro_rules! option {
    ($kind:ident, $label:literal, $description:literal, $icon:literal) => {
        InsertOption {
            kind: ElementKind::$kind,
            label: $label,
            description: $description,
            icon: concat!("icons/", $icon, ".svg"),
            svg: include_bytes!(concat!("../assets/icons/", $icon, ".svg")),
        }
    };
}
const OPTIONS: &[InsertOption] = &[
    option!(Heading, "見出し", "スライドの題名や節の見出し", "heading"),
    option!(Text, "本文", "説明を文章で伝える", "text"),
    option!(Bullets, "箇条書き", "要点を項目ごとに並べる", "bullets"),
    option!(Code, "コード", "ソースコードを等幅で表示", "code"),
    option!(Image, "画像", "画像ファイルを参照して表示", "image"),
    option!(Columns, "2列", "左右の列に内容を並べる", "columns"),
];

impl InsertRequest {
    fn stale_reason(&self, document: &PresentationDocument) -> Option<&'static str> {
        (document.source() != self.source.as_ref())
            .then_some("文書が変わりました。挿入メニューを開き直してください。")
    }
    fn parent_reason(&self, document: &PresentationDocument) -> Option<&'static str> {
        let Some(slide) = document
            .model()
            .and_then(|model| model.slides.get(self.slide))
        else {
            return Some("追加先のスライドがありません。");
        };
        if let InsertParent::Column { columns, .. } = self.parent {
            if self.kind == ElementKind::Columns {
                return Some("列内には2列を追加できません。");
            }
            if !matches!(slide.elements.get(columns), Some(Element::Columns { .. })) {
                return Some("追加先の2列がありません。挿入メニューを開き直してください。");
            }
        }
        None
    }
}

impl PsychoApp {
    fn insert_parents(&self, slide: usize) -> Vec<(InsertParent, String)> {
        let mut parents = vec![(InsertParent::Slide, "スライド直下".into())];
        if let Some(slide) = self
            .document
            .model()
            .and_then(|model| model.slides.get(slide))
        {
            let mut number = 0;
            for (columns, element) in slide.elements.iter().enumerate() {
                if matches!(element, Element::Columns { .. }) {
                    number += 1;
                    for side in [ColumnSide::Left, ColumnSide::Right] {
                        parents.push((
                            InsertParent::Column { columns, side },
                            format!("2列 {number} の{}", side.label()),
                        ));
                    }
                }
            }
        }
        parents
    }

    fn insert_disabled_reason(&self, request: &InsertRequest) -> Option<&'static str> {
        if self.presentation.is_some() {
            return Some("発表を終了してください。");
        }
        if self.external_edit_blocked() || self.document.model().is_none() {
            return Some("文書や外部ファイルの問題を解決してください。");
        }
        request
            .stale_reason(&self.document)
            .or_else(|| request.parent_reason(&self.document))
    }

    pub(super) fn open_insert_menu(
        &mut self,
        parent: InsertParent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.presentation.is_some()
            || self.external_edit_blocked()
            || self.document.model().is_none()
        {
            self.status = "文書や外部ファイルの問題を解決してください。".into();
            cx.notify();
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        self.popup_return_focus = window.focused(cx);
        let parents = self.insert_parents(self.current_slide);
        let focus = (0..parents.len() + OPTIONS.len())
            .map(|_| cx.focus_handle())
            .collect::<Vec<_>>();
        window.focus(&focus[0], cx);
        self.view.popup = Some(Popup::Insert(InsertMenuState {
            slide: self.current_slide,
            parent,
            source: Arc::from(self.document.source()),
            parents,
            focus,
        }));
        cx.notify();
    }

    pub(super) fn insert_element(
        &mut self,
        request: InsertRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(reason) = self.insert_disabled_reason(&request) {
            self.status = reason.into();
            cx.notify();
            return;
        }
        if let Err(error) = self.commit_draft(window, cx) {
            self.status = error;
            cx.notify();
            return;
        }
        if let Some(reason) = request.parent_reason(&self.document) {
            self.status = reason.into();
            cx.notify();
            return;
        }
        let result = match request.parent {
            InsertParent::Slide => self.document.add_element(request.slide, request.kind),
            InsertParent::Column { columns, side } => {
                self.document
                    .add_column_element(request.slide, columns, side.index(), request.kind)
            }
        };
        let index = match result {
            Ok(index) => index,
            Err(error) => {
                self.status = format!("追加できませんでした。{error}");
                cx.notify();
                return;
            }
        };
        self.view.popup = None;
        self.popup_return_focus = None;
        let image = request.kind == ElementKind::Image;
        let field = if image {
            ElementField::ImagePath
        } else {
            ElementField::Text
        };
        if image {
            self.view.inspector.visible = true;
        }
        match request.parent {
            InsertParent::Slide if request.kind == ElementKind::Columns => {
                self.select_columns(request.slide, index, window, cx)
            }
            InsertParent::Slide => {
                self.select_element(request.slide, index, field, !image, window, cx)
            }
            InsertParent::Column { columns, side } => self.select_nested_element(
                request.slide,
                columns,
                side.index(),
                index,
                field,
                !image,
                window,
                cx,
            ),
        }
        if image {
            window.focus(&self.editor.read(cx).focus_handle(), cx);
        }
        self.refresh_assets();
        self.status = "要素を追加しました。".into();
        cx.notify();
    }

    pub(super) fn cycle_insert_focus(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(Popup::Insert(state)) = &self.view.popup else {
            return false;
        };
        if self
            .document
            .model()
            .is_some_and(|model| model.slides.is_empty())
        {
            window.focus(&state.focus[0], cx);
            return true;
        }
        let parents = state.parents.len();
        let handles = state
            .focus
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                *index < parents
                    || OPTIONS.get(index - parents).is_some_and(|option| {
                        self.insert_disabled_reason(&InsertRequest {
                            slide: state.slide,
                            parent: state.parent,
                            kind: option.kind,
                            source: state.source.clone(),
                        })
                        .is_none()
                    })
            })
            .map(|(_, focus)| focus)
            .collect::<Vec<_>>();
        let current = handles.iter().position(|focus| focus.is_focused(window));
        let next = match current {
            Some(index) if backwards => (index + handles.len() - 1) % handles.len(),
            Some(index) => (index + 1) % handles.len(),
            None => 0,
        };
        window.focus(handles[next], cx);
        true
    }

    pub(super) fn render_insert_menu(
        &self,
        state: InsertMenuState,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if self
            .document
            .model()
            .is_some_and(|model| model.slides.is_empty())
        {
            let weak = cx.entity().downgrade();
            let enabled = self.availability(window, cx).edit;
            return div()
                .child("まずスライドを作成してください。")
                .child(
                    div()
                        .id("insert-create-slide")
                        .track_focus(&state.focus[0])
                        .tab_stop(enabled)
                        .role(gpui::Role::Button)
                        .aria_label("スライド追加")
                        .min_h(px(32.))
                        .border_1()
                        .border_color(self.palette.border)
                        .focus(|style| style.border_2().border_color(self.palette.focus))
                        .child("スライド追加")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.execute_action(&AddSlide, window, cx)
                        }))
                        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                            if event.keystroke.key == "enter" {
                                this.execute_action(&AddSlide, window, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                            weak.update(cx, |this, cx| this.execute_action(&AddSlide, window, cx))
                                .ok();
                        }),
                )
                .into_any_element();
        }
        let mut panel = div().flex().flex_col().gap(px(4.)).child("追加先");
        let parents = &state.parents;
        for (index, (parent, label)) in parents.iter().enumerate() {
            let parent = *parent;
            let selected = state.parent == parent;
            let weak = cx.entity().downgrade();
            panel = panel.child(
                div()
                    .id(("insert-parent", index))
                    .accessibility_id(format!("insert-parent-{index}"))
                    .track_focus(&state.focus[index])
                    .tab_stop(true)
                    .role(gpui::Role::RadioButton)
                    .aria_label(label.clone())
                    .aria_selected(selected)
                    .px(px(8.))
                    .min_h(px(32.))
                    .rounded_md()
                    .border_1()
                    .border_color(self.palette.border)
                    .bg(if selected {
                        self.palette.selection
                    } else {
                        self.palette.surface
                    })
                    .focus(|style| style.border_2().border_color(self.palette.focus))
                    .child(format!("{} {label}", if selected { "●" } else { "○" }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_insert_parent(parent, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.choose_insert_parent(parent, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .on_a11y_action(gpui::AccessibleAction::Click, move |_, _, cx| {
                        weak.update(cx, |this, cx| this.choose_insert_parent(parent, cx))
                            .ok();
                    }),
            );
        }
        panel = panel.child(div().pt(px(8.)).child("要素の種類"));
        for (index, option) in OPTIONS.iter().enumerate() {
            let request = InsertRequest {
                slide: state.slide,
                parent: state.parent,
                kind: option.kind,
                source: state.source.clone(),
            };
            let reason = self.insert_disabled_reason(&request);
            let enabled = reason.is_none();
            let click = request.clone();
            let keyboard = request.clone();
            let weak = cx.entity().downgrade();
            panel = panel.child(
                div()
                    .id(("insert-kind", index))
                    .accessibility_id(format!("insert-kind-{index}"))
                    .track_focus(&state.focus[parents.len() + index])
                    .tab_stop(enabled)
                    .role(gpui::Role::Button)
                    .aria_label(option.label)
                    .aria_description(reason.unwrap_or(option.description))
                    .a11y_synthetic_children(move |builder| {
                        if !enabled {
                            builder.parent_node().set_disabled();
                        }
                    })
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .min_h(px(48.))
                    .px(px(8.))
                    .rounded_md()
                    .border_1()
                    .border_color(self.palette.border)
                    .focus(|style| style.border_2().border_color(self.palette.focus))
                    .text_color(if enabled {
                        self.palette.text
                    } else {
                        self.palette.secondary
                    })
                    .child(
                        gpui::svg()
                            .path(option.icon)
                            .size(px(24.))
                            .text_color(if enabled {
                                self.palette.text
                            } else {
                                self.palette.secondary
                            }),
                    )
                    .child(
                        div().flex().flex_col().child(option.label).child(
                            div()
                                .text_size(px(12.))
                                .child(reason.unwrap_or(option.description)),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.insert_element(click.clone(), window, cx);
                    }))
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            if event.keystroke.key == "enter" {
                                this.insert_element(keyboard.clone(), window, cx);
                                cx.stop_propagation();
                            }
                        },
                    ))
                    .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                        weak.update(cx, |this, cx| {
                            this.insert_element(request.clone(), window, cx)
                        })
                        .ok();
                    }),
            );
        }
        panel.into_any_element()
    }

    fn choose_insert_parent(&mut self, parent: InsertParent, cx: &mut Context<Self>) {
        if let Some(Popup::Insert(state)) = &mut self.view.popup {
            state.parent = parent;
            cx.notify();
        }
    }

    pub(super) fn native_insert_menu(&self, window: &Window, cx: &Context<Self>) -> Menu {
        if self
            .document
            .model()
            .is_some_and(|model| model.slides.is_empty())
        {
            return Menu::new("挿入").items([action_menu(AddSlide, self.availability(window, cx))]);
        }
        let source: Arc<str> = Arc::from(self.document.source());
        let items = self
            .insert_parents(self.current_slide)
            .into_iter()
            .map(|(parent, label)| {
                MenuItem::submenu(Menu::new(label).items(OPTIONS.iter().map(|option| {
                    let request = InsertRequest {
                        slide: self.current_slide,
                        parent,
                        kind: option.kind,
                        source: source.clone(),
                    };
                    let disabled = self.insert_disabled_reason(&request).is_some();
                    MenuItem::action(
                        format!("{}  {}", option.label, option.description),
                        InsertElement { request },
                    )
                    .disabled(disabled)
                })))
            });
        Menu::new("挿入").items(items)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn decorate_native_menus() {
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    unsafe fn decorate(menu: *mut Object, images: &[*mut Object]) {
        unsafe {
            let _: () = msg_send![menu, setAutoenablesItems: objc::runtime::NO];
            let count: usize = msg_send![menu, numberOfItems];
            for index in 0..count {
                let item: *mut Object = msg_send![menu, itemAtIndex: index];
                let submenu: *mut Object = msg_send![item, submenu];
                if !submenu.is_null() {
                    decorate(submenu, images);
                } else {
                    let title: *mut Object = msg_send![item, title];
                    let title_bytes: *const std::ffi::c_char = msg_send![title, UTF8String];
                    let title = std::ffi::CStr::from_ptr(title_bytes).to_string_lossy();
                    for (option, image) in OPTIONS.iter().zip(images) {
                        if title.starts_with(&format!("{}  ", option.label)) {
                            let _: () = msg_send![item, setImage: *image];
                        }
                    }
                }
            }
        }
    }
    thread_local! { static IMAGES: Vec<*mut Object> = native_icons(); }
    unsafe {
        let app: *mut Object = msg_send![objc::class!(NSApplication), sharedApplication];
        let menu: *mut Object = msg_send![app, mainMenu];
        if !menu.is_null() {
            IMAGES.with(|images| decorate(menu, images));
        }
    }
}

#[cfg(target_os = "macos")]
fn native_icons() -> Vec<*mut objc::runtime::Object> {
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    let renderer = gpui::SvgRenderer::new(Arc::new(()));
    OPTIONS.iter().map(|option| {
        let rendered = renderer.render_single_frame(option.svg, 2.).expect("bundled insertion icon");
        let dimensions = rendered.size(0);
        let mut rgba = rendered.as_bytes(0).unwrap().to_vec();
        for pixel in rgba.chunks_exact_mut(4) { pixel.swap(0, 2); }
        let image = image::RgbaImage::from_raw(dimensions.width.0 as u32, dimensions.height.0 as u32, rgba).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png).expect("bundled icon PNG");
        let bytes = png.into_inner();
        unsafe {
            let data: *mut Object = msg_send![objc::class!(NSData), dataWithBytes: bytes.as_ptr() length: bytes.len()];
            let image: *mut Object = msg_send![objc::class!(NSImage), alloc];
            let image: *mut Object = msg_send![image, initWithData: data];
            let _: () = msg_send![image, setSize: gpui::Size::new(18f64, 18f64)];
            let _: () = msg_send![image, setTemplate: objc::runtime::YES];
            image
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        document: &PresentationDocument,
        slide: usize,
        parent: InsertParent,
        kind: ElementKind,
    ) -> InsertRequest {
        InsertRequest {
            slide,
            parent,
            kind,
            source: Arc::from(document.source()),
        }
    }
    fn document() -> PresentationDocument {
        PresentationDocument::from_source("presentation {\n metadata { title \"Parents\" }\n slide id=\"a\" {\n columns { column width=50; column width=50 } // A\n columns { column width=50; column width=50 } // B\n }\n slide id=\"b\" {}\n}\n").unwrap()
    }
    #[test]
    fn stale_request_rejects_a_different_columns_at_the_same_index() {
        let mut document = document();
        let old = request(
            &document,
            0,
            InsertParent::Column {
                columns: 0,
                side: ColumnSide::Left,
            },
            ElementKind::Text,
        );
        document.remove_element(0, 0).unwrap();
        assert_eq!(old.parent_reason(&document), None);
        assert_eq!(
            old.stale_reason(&document),
            Some("文書が変わりました。挿入メニューを開き直してください。")
        );
        let after = document.source().to_owned();
        assert!(after.contains("// B"));
        assert!(!after.contains("text"));
        document.undo();
        let restored = request(&document, 0, old.parent, ElementKind::Text);
        document.redo();
        assert!(restored.stale_reason(&document).is_some());
    }
    #[test]
    fn stale_slide_and_reordered_parent_are_rejected() {
        let mut document = document();
        let old = request(&document, 0, InsertParent::Slide, ElementKind::Text);
        document.move_slide(0, 1).unwrap();
        assert_eq!(old.parent_reason(&document), None);
        assert!(old.stale_reason(&document).is_some());
        document.undo();
        let old = request(
            &document,
            0,
            InsertParent::Column {
                columns: 0,
                side: ColumnSide::Right,
            },
            ElementKind::Text,
        );
        document.move_element(0, 0, 1).unwrap();
        assert_eq!(old.parent_reason(&document), None);
        assert!(old.stale_reason(&document).is_some());
    }
    #[test]
    fn columns_nesting_and_missing_parents_are_rejected() {
        let document = document();
        let parent = InsertParent::Column {
            columns: 0,
            side: ColumnSide::Left,
        };
        assert_eq!(
            request(&document, 0, parent, ElementKind::Columns).parent_reason(&document),
            Some("列内には2列を追加できません。")
        );
        assert_eq!(
            request(&document, 9, InsertParent::Slide, ElementKind::Text).parent_reason(&document),
            Some("追加先のスライドがありません。")
        );
        assert_eq!(
            request(&document, 1, parent, ElementKind::Text).parent_reason(&document),
            Some("追加先の2列がありません。挿入メニューを開き直してください。")
        );
    }
}
