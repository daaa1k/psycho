//! Platform IME helpers for the GPUI text input client.

#[cfg(target_os = "macos")]
mod macos {
    use gpui::Window;
    use objc::{
        msg_send,
        runtime::{Object, YES},
        sel, sel_impl,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{cell::RefCell, rc::Rc};

    const CANDIDATE_PANEL_LEVEL: isize = 20;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NativeRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
    unsafe impl objc::Encode for NativeRect {
        fn encode() -> objc::Encoding {
            unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
        }
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NativeRange {
        location: usize,
        length: usize,
    }
    unsafe impl objc::Encode for NativeRange {
        fn encode() -> objc::Encoding {
            unsafe { objc::Encoding::from_str("{_NSRange=QQ}") }
        }
    }
    struct PanelAnchor {
        panel: objc::rc::StrongPtr,
        left: f64,
        top: f64,
    }

    struct CompositionGeometry {
        native_anchor: NativeRect,
        panels: Vec<PanelAnchor>,
    }

    #[derive(Default)]
    struct GeometryState {
        previous_anchor: Option<NativeRect>,
        relocated: Option<CompositionGeometry>,
    }

    #[derive(Clone, Default)]
    pub struct Geometry(Rc<RefCell<GeometryState>>);

    impl Geometry {
        pub fn refresh(&self, window: &Window, geometry_changed: bool) {
            let state = self.0.clone();
            // Query after paint and outside GPUI's Window borrow: AppKit calls back
            // into the input handler while requesting character coordinates.
            window.on_next_frame(move |window, cx| {
                let Ok(handle) = window.window_handle() else {
                    return;
                };
                let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                    return;
                };
                let view = unsafe {
                    objc::rc::StrongPtr::retain(handle.ns_view.as_ptr().cast::<Object>())
                };
                cx.foreground_executor()
                    .spawn(async move {
                        unsafe {
                            let native_window: *mut Object = msg_send![*view, window];
                            if native_window.is_null() {
                                return;
                            }
                            let marked: objc::runtime::BOOL = msg_send![*view, hasMarkedText];
                            if marked != YES {
                                *state.borrow_mut() = GeometryState::default();
                            } else {
                                let marked_range: NativeRange = msg_send![*view, markedRange];
                                let range = NativeRange {
                                    location: marked_range.location,
                                    length: 0,
                                };
                                let rect: NativeRect = msg_send![*view,
                                    firstRectForCharacterRange: range
                                    actualRange: std::ptr::null_mut::<NativeRange>()
                                ];
                                if !rect.x.is_finite() || !rect.y.is_finite() || rect.height <= 0. {
                                    *state.borrow_mut() = GeometryState::default();
                                    return;
                                }
                                let mut geometry = state.borrow_mut();
                                if geometry_changed && geometry.relocated.is_none() {
                                    if let Some(native_anchor) = geometry.previous_anchor {
                                        geometry.relocated = Some(CompositionGeometry {
                                            native_anchor,
                                            panels: Vec::new(),
                                        });
                                    }
                                }
                                geometry.previous_anchor = Some(rect);
                                if let Some(composition) = &mut geometry.relocated {
                                    let application: *mut Object =
                                        msg_send![objc::class!(NSApplication), sharedApplication];
                                    let windows: *mut Object = msg_send![application, windows];
                                    let count: usize = msg_send![windows, count];
                                    let mut panels = Vec::new();
                                    for index in 0..count {
                                        let panel: *mut Object =
                                            msg_send![windows, objectAtIndex: index];
                                        let visible: objc::runtime::BOOL =
                                            msg_send![panel, isVisible];
                                        let is_panel: objc::runtime::BOOL =
                                            msg_send![panel, isKindOfClass: objc::class!(NSPanel)];
                                        let level: isize = msg_send![panel, level];
                                        // Apple's in-process Japanese candidate panels use level 20.
                                        if visible == YES
                                            && is_panel == YES
                                            && level == CANDIDATE_PANEL_LEVEL
                                        {
                                            let frame: NativeRect = msg_send![panel, frame];
                                            panels.push((panel, frame));
                                        }
                                    }
                                    composition.panels.retain(|anchor| {
                                        panels.iter().any(|(panel, _)| *panel == *anchor.panel)
                                    });
                                    // Snapshot all panels before moving any: auxiliary panels can
                                    // follow their primary panel's setFrame call.
                                    for (panel, frame) in &mut panels {
                                        let anchor = if let Some(index) = composition
                                            .panels
                                            .iter()
                                            .position(|anchor| *anchor.panel == *panel)
                                        {
                                            &composition.panels[index]
                                        } else {
                                            composition.panels.push(PanelAnchor {
                                                panel: objc::rc::StrongPtr::retain(*panel),
                                                left: frame.x - composition.native_anchor.x,
                                                top: frame.y + frame.height
                                                    - composition.native_anchor.y,
                                            });
                                            composition.panels.last().unwrap()
                                        };
                                        frame.x = rect.x + anchor.left;
                                        frame.y = rect.y + anchor.top - frame.height;
                                    }
                                    let screen: *mut Object = msg_send![native_window, screen];
                                    if !screen.is_null() && !panels.is_empty() {
                                        let visible: NativeRect = msg_send![screen, visibleFrame];
                                        let left = panels
                                            .iter()
                                            .map(|(_, f)| f.x)
                                            .fold(f64::INFINITY, f64::min);
                                        let bottom = panels
                                            .iter()
                                            .map(|(_, f)| f.y)
                                            .fold(f64::INFINITY, f64::min);
                                        let right = panels
                                            .iter()
                                            .map(|(_, f)| f.x + f.width)
                                            .fold(f64::NEG_INFINITY, f64::max);
                                        let top = panels
                                            .iter()
                                            .map(|(_, f)| f.y + f.height)
                                            .fold(f64::NEG_INFINITY, f64::max);
                                        let adjust_x = (visible.x + visible.width - right)
                                            .min(0.)
                                            .max(visible.x - left);
                                        let adjust_y = (visible.y + visible.height - top)
                                            .min(0.)
                                            .max(visible.y - bottom);
                                        for (_, frame) in &mut panels {
                                            frame.x += adjust_x;
                                            frame.y += adjust_y;
                                        }
                                    }
                                    drop(geometry);
                                    for (panel, frame) in panels {
                                        let _: () = msg_send![panel, setFrame: frame display: YES];
                                    }
                                }
                            }
                            let context: *mut Object = msg_send![*view, inputContext];
                            if !context.is_null() {
                                let _: () = msg_send![context, invalidateCharacterCoordinates];
                            }
                        }
                    })
                    .detach();
            });
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use gpui::Window;

    #[derive(Clone, Default)]
    pub struct Geometry;

    impl Geometry {
        pub fn refresh(&self, _window: &Window, _geometry_changed: bool) {}
    }
}

#[cfg(target_os = "macos")]
pub use macos::Geometry;
#[cfg(not(target_os = "macos"))]
pub use stub::Geometry;
