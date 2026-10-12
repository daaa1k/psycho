use super::*;
use gpui::{ScrollStrategy, UniformListScrollHandle, uniform_list};

actions!(
    psycho,
    [SelectPreviousSlide, SelectNextSlide, FocusSlideCanvas]
);

pub(super) struct NavigatorState {
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    exposure: Option<Exposure>,
}

#[derive(PartialEq)]
struct Exposure {
    selected: Option<usize>,
    count: usize,
    width: f32,
    visible: bool,
}

impl NavigatorState {
    pub(super) fn new(focus: FocusHandle) -> Self {
        Self {
            focus,
            scroll: UniformListScrollHandle::new(),
            exposure: None,
        }
    }

    pub(super) fn focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    fn expose(&mut self, exposure: Exposure) {
        if self.exposure.as_ref() != Some(&exposure) && exposure.visible {
            if let Some(index) = exposure.selected {
                self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            }
        }
        self.exposure = Some(exposure);
    }
}

impl PsychoApp {
    pub(super) fn render_slide_list(
        &mut self,
        model: Option<Arc<PresentationModel>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let count = model.as_ref().map_or(0, |model| model.slides.len());
        let width = self.view.widths(f32::from(window.viewport_size().width)).0;
        self.navigator.expose(Exposure {
            selected: (self.current_slide < count).then_some(self.current_slide),
            count,
            width,
            visible: self.view.navigator.visible,
        });
        let mut list = div()
            .id("navigator")
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .track_focus(&self.navigator.focus)
            .key_context("SlideNavigator")
            .role(gpui::Role::ListBox)
            .aria_label("スライド一覧")
            .aria_description("上下キーで選択、Returnでキャンバスへ移動")
            .on_action(cx.listener(|this, _: &SelectPreviousSlide, window, cx| {
                this.navigate_slide(-1, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectNextSlide, window, cx| {
                this.navigate_slide(1, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusSlideCanvas, window, cx| {
                window.focus(&this.canvas_focus, cx);
                cx.notify();
            }))
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .min_h_0()
            .bg(self.palette.surface)
            .child(
                div()
                    .px(px(8.))
                    .py(px(8.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("スライド"),
            );
        if let Some(model) = model {
            let thumbnail_width = (width - 26.).max(1.);
            let scale = thumbnail_width / 1280.;
            let row_height = thumbnail_width * 9. / 16. + 42.;
            list = list.child(
                uniform_list(
                    "navigator-rows",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        editor_validation::record_navigator_range(
                            range.clone(),
                            scale,
                            this.current_slide,
                        );
                        range
                            .map(|index| {
                                let slide = &model.slides[index];
                                let selected = index == this.current_slide;
                                let focused = selected && this.navigator.focused(window);
                                let heading =
                                    slide.elements.iter().find_map(|element| match element {
                                        Element::Heading(text) if !text.trim().is_empty() => {
                                            Some(text.clone())
                                        }
                                        _ => None,
                                    });
                                let fallback = format!("スライド {}", index + 1);
                                let label = heading.as_ref().map_or_else(
                                    || fallback.clone(),
                                    |heading| format!("スライド {} {}", index + 1, heading),
                                );
                                let heading = heading.unwrap_or(fallback);
                                let tooltip: gpui::SharedString = heading.clone().into();
                                let identity = slide.id.as_ref().map_or_else(
                                    || format!("index-{index}"),
                                    |id| format!("id-{id}"),
                                );
                                let weak_click = cx.entity().downgrade();
                                let weak_focus = weak_click.clone();
                                let thumbnail = this.render_slide_body(
                                    Some(slide),
                                    index,
                                    scale,
                                    SlideInteraction::ReadOnly,
                                    &this.asset_images,
                                    window,
                                    cx,
                                );
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "navigator-slide-{identity}"
                                    )))
                                    .accessibility_id(format!("navigator-slide-{identity}"))
                                    .role(gpui::Role::ListBoxOption)
                                    .aria_label(label)
                                    .aria_selected(selected)
                                    .when(selected, |row| row.aria_active_descendant())
                                    .h(px(row_height))
                                    .w_full()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .p(px(2.))
                                            .border_2()
                                            .border_color(if focused {
                                                this.palette.focus
                                            } else {
                                                this.palette.surface
                                            })
                                            .bg(if selected {
                                                this.palette.selection
                                            } else {
                                                this.palette.surface
                                            })
                                            .child(
                                                div()
                                                    .border_1()
                                                    .border_color(if selected {
                                                        this.palette.focus
                                                    } else {
                                                        this.palette.border
                                                    })
                                                    .w(px(thumbnail_width + 2.))
                                                    .h(px(thumbnail_width * 9. / 16. + 2.))
                                                    .overflow_hidden()
                                                    .child(thumbnail),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(6.))
                                            .h(px(24.))
                                            .items_center()
                                            .text_size(px(12.))
                                            .child(div().flex_shrink_0().child(format!(
                                                "{}{}",
                                                if selected { "› " } else { "" },
                                                index + 1
                                            )))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_ellipsis()
                                                    .child(heading),
                                            ),
                                    )
                                    .tooltip(move |_, cx| {
                                        cx.new(|_| EditorTooltip(tooltip.clone())).into()
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                                        window.prevent_default()
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_navigator_slide(index, window, cx)
                                    }))
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Click,
                                        move |_, window, cx| {
                                            weak_click
                                                .update(cx, |this, cx| {
                                                    this.select_navigator_slide(index, window, cx)
                                                })
                                                .ok();
                                        },
                                    )
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Focus,
                                        move |_, window, cx| {
                                            weak_focus
                                                .update(cx, |this, cx| {
                                                    this.select_navigator_slide(index, window, cx)
                                                })
                                                .ok();
                                        },
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.navigator.scroll)
                .flex_1()
                .min_h_0()
                .w_full(),
            );
        }
        list.into_any_element()
    }

    fn navigate_slide(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.document.model().map_or(0, |model| model.slides.len());
        if count == 0 {
            return;
        }
        let index = self
            .current_slide
            .saturating_add_signed(delta)
            .min(count - 1);
        self.select_navigator_slide(index, window, cx);
    }

    fn select_navigator_slide(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .document
            .model()
            .and_then(|model| model.slides.get(index))
            .is_none()
        {
            return;
        }
        if !self.external_edit_blocked() {
            let input = self.editor.read(cx);
            if self.has_uncommitted_draft(cx) || input.can_undo() || input.can_redo() {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return;
                }
            }
            self.target = EditTarget::Title;
            self.canvas_editing = false;
            self.column_add_menu = None;
            let title = self.edit_value(EditTarget::Title);
            self.editor.update(cx, |input, cx| {
                input.set_multiline(false, cx);
                input.set_value(&title, cx);
            });
        }
        self.current_slide = index;
        self.navigator
            .scroll
            .scroll_to_item(index, ScrollStrategy::Nearest);
        window.focus(&self.navigator.focus, cx);
        cx.notify();
    }
}
