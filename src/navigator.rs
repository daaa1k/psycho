use super::*;
use gpui::{ScrollStrategy, UniformListScrollHandle, uniform_list};

actions!(
    psycho,
    [
        SelectPreviousSlide,
        SelectNextSlide,
        SelectAboveSlide,
        SelectBelowSlide,
        FocusSlideCanvas
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CollectionHost {
    Navigator,
    Overview,
}
impl CollectionHost {
    fn prefix(self) -> &'static str {
        match self {
            Self::Navigator => "navigator",
            Self::Overview => "overview",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CollectionGeometry {
    columns: usize,
    cell_width: f32,
    row_height: f32,
    count: usize,
}
impl CollectionGeometry {
    fn new(host: CollectionHost, width: f32, count: usize) -> Self {
        let columns = match host {
            CollectionHost::Navigator => 1,
            CollectionHost::Overview => ((width / 240.).floor() as usize).max(1),
        };
        let cell_width = width / columns as f32;
        Self {
            columns,
            cell_width,
            row_height: (cell_width - 26.).max(1.) * 9. / 16. + 42.,
            count,
        }
    }
    fn rows(self) -> usize {
        self.count.div_ceil(self.columns)
    }
    fn selection(self, from: usize, delta: isize) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        if delta < -1 && from < delta.unsigned_abs() {
            return Some(from);
        }
        if delta > 1 && from / self.columns == (self.count - 1) / self.columns {
            return Some(from);
        }
        Some(from.saturating_add_signed(delta).min(self.count - 1))
    }
    fn insertion(self, x: f32, y: f32) -> usize {
        let row = (y.max(0.) / self.row_height).floor() as usize;
        let column = ((x / self.cell_width).round().max(0.) as usize).min(self.columns);
        (row * self.columns + column).min(self.count)
    }
}

pub(super) struct NavigatorState {
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    exposure: Option<Exposure>,
    geometry: Option<CollectionGeometry>,
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
    geometry: CollectionGeometry,
    window_size: gpui::Size<gpui::Pixels>,
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
            geometry: None,
            drag: None,
            drag_generation: 0,
        }
    }

    pub(super) fn columns(&self) -> usize {
        self.geometry.map_or(1, |g| g.columns)
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
                self.scroll.scroll_to_item(
                    index / self.geometry.map_or(1, |g| g.columns),
                    ScrollStrategy::Nearest,
                );
            }
        }
        self.exposure = Some(exposure);
    }
}

impl PsychoApp {
    fn collection(&self, host: CollectionHost) -> &NavigatorState {
        match host {
            CollectionHost::Navigator => &self.navigator,
            CollectionHost::Overview => &self.overview,
        }
    }
    fn collection_mut(&mut self, host: CollectionHost) -> &mut NavigatorState {
        match host {
            CollectionHost::Navigator => &mut self.navigator,
            CollectionHost::Overview => &mut self.overview,
        }
    }
    pub(super) fn collection_return_focus(&self, window: &Window) -> Option<FocusHandle> {
        if self.navigator.focused(window) {
            Some(self.navigator.focus_handle())
        } else if self.overview.focused(window) || self.view.mode == EditorMode::Overview {
            Some(self.overview.focus_handle())
        } else {
            None
        }
    }
    pub(super) fn cancel_slide_drags(&mut self) {
        self.navigator.drag = None;
        self.overview.drag = None;
    }
    pub(super) fn set_editor_mode(
        &mut self,
        mode: EditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.view.mode == mode {
            return true;
        }
        if self.external_edit_blocked() && self.has_uncommitted_draft(cx) {
            self.status = "外部変更との競合があるため入力を確定できません。表示を切り替える前に競合を解決してください。".into();
            cx.notify();
            return false;
        }
        if !self.external_edit_blocked() {
            if let Err(error) = self.commit_draft(window, cx) {
                self.status = error;
                cx.notify();
                return false;
            }
        }
        self.cancel_slide_drags();
        cx.stop_active_drag(window);
        self.view.popup = None;
        self.view.resize = None;
        self.view.mode = mode;
        let focus = if mode == EditorMode::Overview {
            self.overview.focus_handle()
        } else {
            self.canvas_focus.clone()
        };
        window.focus(&focus, cx);
        cx.notify();
        true
    }
    pub(super) fn render_slide_list(
        &mut self,
        model: Option<Arc<PresentationModel>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.render_slide_collection(CollectionHost::Navigator, model, window, cx)
    }
    pub(super) fn render_slide_collection(
        &mut self,
        host: CollectionHost,
        model: Option<Arc<PresentationModel>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let count = model.as_ref().map_or(0, |model| model.slides.len());
        let width = if host == CollectionHost::Navigator {
            self.view.widths(f32::from(window.viewport_size().width)).0
        } else {
            f32::from(self.view.canvas_area.width).max(1.)
        };
        let geometry = CollectionGeometry::new(host, width, count);
        if self.collection(host).geometry != Some(geometry) {
            if self.collection_mut(host).drag.take().is_some() {
                cx.stop_active_drag(window);
            }
            cx.notify();
        }
        self.collection_mut(host).geometry = Some(geometry);
        let selected = self.current_slide;
        let visible = host == CollectionHost::Overview || self.view.navigator.visible;
        self.collection_mut(host).expose(Exposure {
            selected: (selected < count).then_some(selected),
            count,
            width,
            visible,
        });
        if self.collection(host).drag.as_ref().is_some_and(|drag| {
            drag.item.source.as_ref() != self.document.source()
                || drag.window_size != window.viewport_size()
        }) {
            self.collection_mut(host).drag = None;
            cx.stop_active_drag(window);
        }
        let source: Arc<str> = self.document.source().into();
        let mut list = div()
            .id(host.prefix())
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .track_focus(&self.collection(host).focus)
            .key_context(if host == CollectionHost::Overview {
                "SlideOverview"
            } else {
                "SlideNavigator"
            })
            .role(gpui::Role::ListBox)
            .aria_label(if host == CollectionHost::Overview {
                "Overviewのスライド一覧"
            } else {
                "スライド一覧"
            })
            .aria_description(if host == CollectionHost::Overview {
                "左右上下キーで選択、Returnで選択したスライドの編集へ戻る"
            } else {
                "上下キーで選択、Returnでキャンバスへ移動"
            })
            .on_action(
                cx.listener(move |this, _: &SelectPreviousSlide, window, cx| {
                    this.navigate_slide(host, -1, window, cx)
                }),
            )
            .on_action(cx.listener(move |this, _: &SelectNextSlide, window, cx| {
                this.navigate_slide(host, 1, window, cx)
            }))
            .on_action(cx.listener(move |this, _: &SelectAboveSlide, window, cx| {
                let columns = this.collection(host).geometry.map_or(1, |g| g.columns);
                this.navigate_slide(host, -(columns as isize), window, cx);
            }))
            .on_action(cx.listener(move |this, _: &SelectBelowSlide, window, cx| {
                let columns = this.collection(host).geometry.map_or(1, |g| g.columns);
                this.navigate_slide(host, columns as isize, window, cx);
            }))
            .on_action(cx.listener(move |this, _: &FocusSlideCanvas, window, cx| {
                if host == CollectionHost::Overview && count == 0 {
                    return;
                }
                if this.view.mode == EditorMode::Overview {
                    if count == 0 {
                        return;
                    }
                    this.set_editor_mode(EditorMode::Edit, window, cx);
                } else {
                    window.focus(&this.canvas_focus, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(move |this, _: &ClosePopup, window, cx| {
                if this.collection_mut(host).drag.take().is_some() {
                    cx.stop_active_drag(window);
                    cx.notify();
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
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
                    .child(if host == CollectionHost::Overview {
                        "Overview"
                    } else {
                        "スライド"
                    }),
            );
        if let Some(model) = model {
            let thumbnail_width = (geometry.cell_width - 26.).max(1.);
            let scale = thumbnail_width / 1280.;
            let row_height = geometry.row_height;
            let drag_source = source.clone();
            list = list.child(
                uniform_list(
                    gpui::SharedString::from(format!("{}-rows", host.prefix())),
                    geometry.rows(),
                    cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        if host == CollectionHost::Navigator { editor_validation::record_navigator_range(range.clone(), scale, this.current_slide); }
                        editor_validation::record_collection_range(host.prefix(), range.clone(), geometry.columns, count, this.current_slide);
                        let last_visible_row = range.end.saturating_sub(1);
                        range
                            .map(|row| {
                                let cards = (row * geometry.columns..((row + 1) * geometry.columns).min(count)).map(|index| {
                                let slide = &model.slides[index];
                                let selected = index == this.current_slide;
                                let focused = selected && this.collection(host).focused(window);
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
                                    this.collection(host).drag.as_ref().and_then(|drag| drag.insertion);
                                let dragging = this.collection(host)
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
                                        "{}-slide-{identity}", host.prefix()
                                    )))
                                    .accessibility_id(format!("{}-slide-{identity}", host.prefix()))
                                    .role(gpui::Role::ListBoxOption)
                                    .aria_label(label)
                                    .aria_selected(selected)
                                    .when(selected, |row| row.aria_active_descendant())
                                    .h(px(row_height))
                                    .w(px(geometry.cell_width))
                                    .px(px(8.))
                                    .py(px(4.))
                                    .cursor_pointer()
                                    .when(dragging, |row| row.opacity(0.45))
                                    .when(insertion == Some(index), |row| {
                                        if host == CollectionHost::Overview { row.border_l_2().border_color(this.palette.focus) } else { row.border_t_2().border_color(this.palette.focus) }
                                    })
                                    .when(insertion == Some(index + 1) && (index + 1 == count || (host == CollectionHost::Overview && row == last_visible_row && (index + 1) % geometry.columns == 0)), |row| {
                                        if host == CollectionHost::Overview { row.border_r_2().border_color(this.palette.focus) } else { row.border_b_2().border_color(this.palette.focus) }
                                    })
                                    .on_drag(dragged, move |dragged, _, window, cx| {
                                        weak_drag
                                            .update(cx, |this, cx| {
                                                window.focus(&this.collection(host).focus, cx);
                                                this.navigator.drag = None; this.overview.drag = None;
                                                this.collection_mut(host).drag_generation += 1;
                                                let bounds = this.collection(host).scroll.0.borrow().base_handle.bounds();
                                                this.collection_mut(host).drag = Some(SlideDrag {
                                                    item: dragged.clone(),
                                                    pointer: window.mouse_position(),
                                                    bounds,
                                                    geometry,
                                                    window_size: window.viewport_size(),
                                                    insertion: None,
                                                });
                                                this.schedule_slide_drag(host, window, cx);
                                                cx.notify();
                                            })
                                            .ok();
                                        cx.new(|_| SlideDragGhost(dragged.from))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(
                                            move |this, event: &MouseDownEvent, window, cx| {
                                                if this.select_collection_slide(host, index, window, cx) {
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
                                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                                        if this.select_collection_slide(host, index, window, cx) && host == CollectionHost::Overview && event.click_count() >= 2 { this.set_editor_mode(EditorMode::Edit, window, cx); }
                                    }))
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Click,
                                        move |_, window, cx| {
                                            weak_click
                                                .update(cx, |this, cx| {
                                                    this.select_collection_slide(host, index, window, cx);
                                                })
                                                .ok();
                                        },
                                    )
                                    .on_a11y_action(
                                        gpui::AccessibleAction::Focus,
                                        move |_, window, cx| {
                                            weak_focus
                                                .update(cx, |this, cx| {
                                                    this.select_collection_slide(host, index, window, cx);
                                                })
                                                .ok();
                                        },
                                    )
                                }).collect::<Vec<_>>();
                                div().flex().h(px(row_height)).children(cards)
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.collection(host).scroll)
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(move |_, _, window, cx| {
                        cx.defer_in(window, move |this, _, cx| {
                            if this.collection_mut(host).drag.take().is_some() {
                                cx.notify();
                            }
                        });
                    }),
                )
                .on_drag_move(
                    cx.listener(move |this, event: &DragMoveEvent<DraggedSlide>, _, cx| {
                        if let Some(drag) = this.collection_mut(host).drag.as_mut() {
                            drag.pointer = event.event.position;
                            drag.bounds = event.bounds;
                        }
                        this.update_slide_drag(host, false, cx);
                    }),
                )
                .on_drop(cx.listener(move |this, _: &DraggedSlide, window, cx| {
                    let Some(drag) = this.collection_mut(host).drag.take() else {
                        return;
                    };
                    if drag.window_size != window.viewport_size() {
                        cx.notify();
                        return;
                    }
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
            if host == CollectionHost::Overview && self.document.model().is_none() {
                list = list.child(diagnostics_view(
                    self.document.diagnostics(),
                    self.source_selection.as_ref(),
                    cx,
                ));
            } else {
                list = list.child(self.action_button(AddSlide, false, window, cx));
            }
        }
        if host == CollectionHost::Overview {
            let weak = cx.entity().downgrade();
            list = list.relative().child(
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
            );
        }
        list.into_any_element()
    }

    pub(super) fn update_slide_drag_pointer(&mut self, pointer: gpui::Point<gpui::Pixels>) {
        for state in [&mut self.navigator, &mut self.overview] {
            if let Some(drag) = state.drag.as_mut() {
                drag.pointer = pointer;
            }
        }
    }

    fn update_slide_drag(&mut self, host: CollectionHost, scroll: bool, cx: &mut Context<Self>) {
        let state = self.collection_mut(host);
        let Some(drag) = state.drag.as_mut() else {
            return;
        };
        if !drag.bounds.contains(&drag.pointer) {
            drag.insertion = None;
            cx.notify();
            return;
        }
        let handle = state.scroll.0.borrow().base_handle.clone();
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
            let minimum = (height - drag.geometry.row_height * drag.geometry.rows() as f32).min(0.);
            offset.y = px((f32::from(offset.y) + delta).clamp(minimum, 0.));
            handle.set_offset(offset);
        }
        let content_y = y - f32::from(handle.offset().y);
        drag.insertion = Some(if host == CollectionHost::Navigator {
            ((content_y / drag.geometry.row_height).round().max(0.) as usize)
                .min(drag.geometry.count)
        } else {
            drag.geometry
                .insertion(f32::from(drag.pointer.x - drag.bounds.origin.x), content_y)
        });
        cx.notify();
    }

    fn schedule_slide_drag(
        &mut self,
        host: CollectionHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = self.collection(host).drag_generation;
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
                                if this.collection(host).drag.is_none()
                                    || this.collection(host).drag_generation != generation
                                {
                                    return false;
                                }
                                this.update_slide_drag(host, true, cx);
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

    fn navigate_slide(
        &mut self,
        host: CollectionHost,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(geometry) = self.collection(host).geometry else {
            return;
        };
        let Some(index) = geometry.selection(self.current_slide, delta) else {
            return;
        };
        self.select_collection_slide(host, index, window, cx);
    }

    fn select_collection_slide(
        &mut self,
        host: CollectionHost,
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
            let title = self.edit_value(EditTarget::Title);
            self.editor.update(cx, |input, cx| {
                input.set_multiline(false, cx);
                input.set_value(&title, cx);
            });
        }
        self.current_slide = index;
        let state = self.collection(host);
        state.scroll.scroll_to_item(
            index / state.geometry.map_or(1, |g| g.columns),
            ScrollStrategy::Nearest,
        );
        window.focus(&state.focus, cx);
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_navigation_stays_in_edge_rows_and_clamps_short_final_row() {
        let grid = CollectionGeometry::new(CollectionHost::Overview, 960., 10);
        assert_eq!(grid.rows(), 3);
        assert_eq!(grid.selection(2, -4), Some(2));
        assert_eq!(grid.selection(3, 4), Some(7));
        assert_eq!(grid.selection(7, 4), Some(9));
        assert_eq!(grid.selection(8, 4), Some(8));
        assert_eq!(grid.selection(9, -4), Some(5));
        assert_eq!(grid.selection(4, -1), Some(3));
        assert_eq!(grid.selection(3, 1), Some(4));
        assert_eq!(
            CollectionGeometry::new(CollectionHost::Overview, 100., 0).selection(0, 1),
            None
        );
    }
    #[test]
    fn grid_insertion_uses_card_centers_and_clamps_final_row_space() {
        let grid = CollectionGeometry::new(CollectionHost::Overview, 960., 10);
        assert_eq!(grid.insertion(10., 10.), 0);
        assert_eq!(grid.insertion(130., 10.), 1);
        assert_eq!(grid.insertion(239., 10.), 1);
        assert_eq!(grid.insertion(241., 10.), 1);
        assert_eq!(grid.insertion(950., 10.), 4);
        assert_eq!(grid.insertion(10., grid.row_height + 1.), 4);
        assert_eq!(grid.insertion(950., grid.row_height * 2. + 1.), 10);
        assert_eq!(
            CollectionGeometry::new(CollectionHost::Overview, 470., 10).rows(),
            10
        );
    }
}
