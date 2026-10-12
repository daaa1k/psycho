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
    drag: Option<SlideDrag>,
    drag_generation: u64,
}

#[derive(Clone)]
struct DraggedSlide {
    from: usize,
    source: Arc<str>,
}

struct SlideDragGhost(usize);

impl gpui::Render for SlideDragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .child(format!("スライド {} を移動", self.0 + 1))
    }
}

struct SlideDrag {
    item: DraggedSlide,
    pointer: gpui::Point<gpui::Pixels>,
    bounds: gpui::Bounds<gpui::Pixels>,
    row_height: f32,
    count: usize,
    insertion: Option<usize>,
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
            drag: None,
            drag_generation: 0,
        }
    }

    pub(super) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub(super) fn focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    fn expose(&mut self, exposure: Exposure) {
        if self.exposure.as_ref() != Some(&exposure) && exposure.visible && self.drag.is_none() {
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
        let source: Arc<str> = self.document.source().into();
        let mut list = div()
            .id("navigator")
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
            .on_action(cx.listener(|this, _: &ClosePopup, window, cx| {
                if this.navigator.drag.take().is_some() {
                    cx.stop_active_drag(window);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|_, _, window, cx| {
                    cx.defer_in(window, |this, _, cx| {
                        if this.navigator.drag.take().is_some() {
                            cx.notify();
                        }
                    });
                }),
            )
            .flex()
            .flex_col()
            .size_full()
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
            let drag_source = source.clone();
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
                                let weak_drag = weak_click.clone();
                                let dragged = DraggedSlide {
                                    from: index,
                                    source: drag_source.clone(),
                                };
                                let insertion =
                                    this.navigator.drag.as_ref().and_then(|drag| drag.insertion);
                                let dragging = this
                                    .navigator
                                    .drag
                                    .as_ref()
                                    .is_some_and(|drag| drag.item.from == index);
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
                                    .when(dragging, |row| row.opacity(0.45))
                                    .when(insertion == Some(index), |row| {
                                        row.border_t_2().border_color(this.palette.focus)
                                    })
                                    .when(index + 1 == count && insertion == Some(count), |row| {
                                        row.border_b_2().border_color(this.palette.focus)
                                    })
                                    .on_drag(dragged, move |dragged, _, window, cx| {
                                        weak_drag
                                            .update(cx, |this, cx| {
                                                window.focus(&this.navigator.focus, cx);
                                                this.navigator.drag_generation += 1;
                                                this.navigator.drag = Some(SlideDrag {
                                                    item: dragged.clone(),
                                                    pointer: window.mouse_position(),
                                                    bounds: this
                                                        .navigator
                                                        .scroll
                                                        .0
                                                        .borrow()
                                                        .base_handle
                                                        .bounds(),
                                                    row_height,
                                                    count,
                                                    insertion: None,
                                                });
                                                this.schedule_slide_drag(window, cx);
                                                cx.notify();
                                            })
                                            .ok();
                                        cx.new(|_| SlideDragGhost(dragged.from))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(
                                            move |this, event: &MouseDownEvent, window, cx| {
                                                if this.select_navigator_slide(index, window, cx) {
                                                    this.show_popup(
                                                        Popup::Slide(event.position),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            },
                                        ),
                                    )
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
                                        this.select_navigator_slide(index, window, cx);
                                    }))
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Click,
                                        move |_, window, cx| {
                                            weak_click
                                                .update(cx, |this, cx| {
                                                    this.select_navigator_slide(index, window, cx);
                                                })
                                                .ok();
                                        },
                                    )
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Focus,
                                        move |_, window, cx| {
                                            weak_focus
                                                .update(cx, |this, cx| {
                                                    this.select_navigator_slide(index, window, cx);
                                                })
                                                .ok();
                                        },
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.navigator.scroll)
                .on_drag_move(
                    cx.listener(|this, event: &DragMoveEvent<DraggedSlide>, _, cx| {
                        if let Some(drag) = this.navigator.drag.as_mut() {
                            drag.pointer = event.event.position;
                            drag.bounds = event.bounds;
                        }
                        this.update_slide_drag(false, cx);
                    }),
                )
                .on_drop(cx.listener(|this, _: &DraggedSlide, window, cx| {
                    let Some(drag) = this.navigator.drag.take() else {
                        return;
                    };
                    if let Some(boundary) = drag.insertion {
                        let to = boundary.saturating_sub(usize::from(boundary > drag.item.from));
                        if to != drag.item.from {
                            if this.document.source() != drag.item.source.as_ref() {
                                this.status =
                                    "ドラッグ中に文書が変更されたため移動を取り消しました。".into();
                            } else {
                                this.move_slide_to(drag.item.from, to, window, cx);
                            }
                        }
                    }
                    cx.notify();
                    cx.stop_propagation();
                }))
                .flex_1()
                .min_h_0()
                .w_full(),
            );
        }
        if count == 0 {
            list = list.child(self.action_button(AddSlide, false, window, cx));
        }
        list.into_any_element()
    }

    pub(super) fn update_slide_drag_pointer(&mut self, pointer: gpui::Point<gpui::Pixels>) {
        if let Some(drag) = self.navigator.drag.as_mut() {
            drag.pointer = pointer;
        }
    }

    fn update_slide_drag(&mut self, scroll: bool, cx: &mut Context<Self>) {
        let Some(drag) = self.navigator.drag.as_mut() else {
            return;
        };
        if !drag.bounds.contains(&drag.pointer) {
            drag.insertion = None;
            cx.notify();
            return;
        }
        let handle = self.navigator.scroll.0.borrow().base_handle.clone();
        let y = f32::from(drag.pointer.y - drag.bounds.origin.y);
        if scroll {
            let height = f32::from(drag.bounds.size.height);
            let delta = if y < 36. {
                24.
            } else if y > height - 36. {
                -24.
            } else {
                0.
            };
            let mut offset = handle.offset();
            let minimum = (height - drag.row_height * drag.count as f32).min(0.);
            offset.y = px((f32::from(offset.y) + delta).clamp(minimum, 0.));
            handle.set_offset(offset);
        }
        let content_y = y - f32::from(handle.offset().y);
        drag.insertion = Some((content_y / drag.row_height).round().max(0.) as usize);
        drag.insertion = drag.insertion.map(|position| position.min(drag.count));
        cx.notify();
    }

    fn schedule_slide_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.navigator.drag_generation;
        let view = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(30))
                        .await;
                    let keep = cx
                        .update(|_, cx| {
                            view.update(cx, |this, cx| {
                                if this.navigator.drag.is_none()
                                    || this.navigator.drag_generation != generation
                                {
                                    return false;
                                }
                                this.update_slide_drag(true, cx);
                                true
                            })
                        })
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
            .detach();
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
    ) -> bool {
        if self
            .document
            .model()
            .and_then(|model| model.slides.get(index))
            .is_none()
        {
            return false;
        }
        if !self.external_edit_blocked() {
            let input = self.editor.read(cx);
            if self.has_uncommitted_draft(cx) || input.can_undo() || input.can_redo() {
                if let Err(error) = self.commit_draft(window, cx) {
                    self.status = error;
                    cx.notify();
                    return false;
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
        true
    }
}
