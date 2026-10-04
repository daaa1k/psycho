//! macOS candidate geometry for the GPUI text input client.
use gpui::Window;
use objc::{
    msg_send,
    runtime::{Object, YES},
    sel, sel_impl,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::Cell, rc::Rc};

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
unsafe fn native_selected_rect(view: &Object) -> NativeRect {
    unsafe {
        let range: NativeRange = msg_send![view, selectedRange];
        let rect: NativeRect = msg_send![view, firstRectForCharacterRange: range actualRange: std::ptr::null_mut::<NativeRange>()];
        rect
    }
}

#[derive(Clone, Copy, Default)]
struct GeometryState {
    rect: Option<NativeRect>,
    clamp_x: f64,
    clamp_y: f64,
}

#[derive(Clone, Default)]
pub struct Geometry(Rc<Cell<GeometryState>>);

impl Geometry {
    pub fn refresh(&self, window: &Window, geometry_changed: bool) {
        let last_rect = self.0.clone();
        // Query after paint and outside GPUI's Window borrow: AppKit calls back
        // into the input handler while requesting character coordinates.
        window.on_next_frame(move |window, cx| {
            let Ok(handle) = window.window_handle() else {
                return;
            };
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return;
            };
            let view =
                unsafe { objc::rc::StrongPtr::retain(handle.ns_view.as_ptr().cast::<Object>()) };
            cx.foreground_executor()
                .spawn(async move {
                    unsafe {
                        let native_window: *mut Object = msg_send![*view, window];
                        if native_window.is_null() {
                            return;
                        }
                        let rect = native_selected_rect(&**view);
                        if !rect.x.is_finite() || !rect.y.is_finite() || rect.height <= 0. {
                            last_rect.set(GeometryState::default());
                            return;
                        }
                        let previous = last_rect.replace(GeometryState {
                            rect: Some(rect),
                            ..GeometryState::default()
                        });
                        let marked: objc::runtime::BOOL = msg_send![*view, hasMarkedText];
                        if geometry_changed && marked == YES {
                            if let Some(previous_rect) = previous.rect {
                                // Undo the last screen-edge correction before anchoring
                                // the panels to a different surface or window size.
                                let dx = rect.x - previous_rect.x - previous.clamp_x;
                                let dy = rect.y - previous_rect.y - previous.clamp_y;
                                let application: *mut Object =
                                    msg_send![objc::class!(NSApplication), sharedApplication];
                                let windows: *mut Object = msg_send![application, windows];
                                let count: usize = msg_send![windows, count];
                                // Apple's Japanese IME hosts candidate NSPanel windows
                                // in this process at level 20. PSYCHO creates no other
                                // panels at this level. Never touch another app's windows.
                                let mut panels = Vec::new();
                                for index in 0..count {
                                    let panel: *mut Object =
                                        msg_send![windows, objectAtIndex: index];
                                    let visible: objc::runtime::BOOL = msg_send![panel, isVisible];
                                    let is_panel: objc::runtime::BOOL =
                                        msg_send![panel, isKindOfClass: objc::class!(NSPanel)];
                                    let level: isize = msg_send![panel, level];
                                    if visible == YES
                                        && is_panel == YES
                                        && level == CANDIDATE_PANEL_LEVEL
                                    {
                                        let mut frame: NativeRect = msg_send![panel, frame];
                                        frame.x += dx;
                                        frame.y += dy;
                                        panels.push((panel, frame));
                                    }
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
                                    last_rect.set(GeometryState {
                                        rect: Some(rect),
                                        clamp_x: adjust_x,
                                        clamp_y: adjust_y,
                                    });
                                    for (_, frame) in &mut panels {
                                        frame.x += adjust_x;
                                        frame.y += adjust_y;
                                    }
                                }
                                // A candidate's auxiliary panel may move with the primary
                                // panel. Snapshot every frame before changing any of them.
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
