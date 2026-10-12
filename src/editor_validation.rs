use super::*;

pub(super) fn record_navigator_range(range: std::ops::Range<usize>, scale: f32, selected: usize) {
    let Some(path) = std::env::var_os("PSYCHO_NAVIGATOR_TRACE") else {
        return;
    };
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{},{},{},{}", range.start, range.end, scale, selected);
    }
}

#[allow(unused_variables)]
pub(super) fn configure_appearance(window: &Window) {
    #[cfg(target_os = "macos")]
    configure_appearance_macos(window);
}

#[cfg(target_os = "macos")]
fn configure_appearance_macos(window: &Window) {
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let name = match std::env::var("PSYCHO_ACCEPTANCE_APPEARANCE").as_deref() {
        Ok("light") => c"NSAppearanceNameAqua",
        Ok("dark") => c"NSAppearanceNameDarkAqua",
        _ => return,
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<Object>();
        let native_window: *mut Object = msg_send![view, window];
        let name: *mut Object =
            msg_send![objc::class!(NSString), stringWithUTF8String: name.as_ptr()];
        let appearance: *mut Object = msg_send![objc::class!(NSAppearance), appearanceNamed: name];
        let _: () = msg_send![native_window, setAppearance: appearance];
    }
}

pub(super) fn record(app: &PsychoApp, window: &Window, cx: &Context<PsychoApp>) {
    let Some(path) = std::env::var_os("PSYCHO_EDITOR_STATE") else {
        return;
    };
    let source = app
        .document
        .source()
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let (navigator, inspector) = app.view.widths(f32::from(window.viewport_size().width));
    let snapshot = format!(
        "{{\"revision\":\"{}\",\"sourceHex\":\"{}\",\"dirty\":{},\"undo\":{},\"redo\":{},\"draft\":{},\"navigator\":{},\"inspector\":{},\"navigatorWidth\":{},\"inspectorWidth\":{},\"canvasWidth\":{},\"canvasHeight\":{},\"scale\":{},\"actual\":{},\"presenting\":{},\"dark\":{},\"currentSlide\":{},\"navigatorFocused\":{},\"canvasFocused\":{}}}\n",
        option_env!("PSYCHO_BUILD_REVISION").unwrap_or("unrecorded"),
        source,
        app.document.is_dirty(),
        app.document.can_undo(),
        app.document.can_redo(),
        app.has_uncommitted_draft(cx),
        app.view.navigator.visible,
        app.view.inspector.visible,
        navigator,
        inspector,
        f32::from(app.view.canvas_area.width),
        f32::from(app.view.canvas_area.height),
        app.view.scale(),
        app.view.zoom == ZoomMode::Actual,
        app.presentation.is_some(),
        matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        ),
        app.current_slide,
        app.navigator.focused(window),
        app.canvas_focus.is_focused(window),
    );
    let _ = std::fs::write(path, snapshot);
}
