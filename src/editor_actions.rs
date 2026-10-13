use super::*;

actions!(
    psycho,
    [
        HideApp,
        SaveFile,
        SaveAs,
        UndoEdit,
        RedoEdit,
        NextSlide,
        PreviousSlide,
        ExitPresentation,
        DeleteSelection,
        CommitTextEdit,
        OpenFile,
        AddSlide,
        DeleteSlide,
        MoveSlideEarlier,
        MoveSlideLater,
        ShowInsertMenu,
        ShowViewMenu,
        ToggleNavigator,
        ToggleInspector,
        NarrowNavigator,
        WidenNavigator,
        NarrowInspector,
        WidenInspector,
        FitCanvas,
        ActualSize,
        PresentCurrent,
        PresentFirst,
        FocusNext,
        FocusPrevious,
        ClosePopup
    ]
);

#[derive(Clone, Copy)]
pub(super) struct EditorAvailability {
    pub(super) open: bool,
    pub(super) save: bool,
    pub(super) save_as: bool,
    pub(super) undo: bool,
    pub(super) redo: bool,
    pub(super) edit: bool,
    pub(super) delete_slide: bool,
    pub(super) earlier: bool,
    pub(super) later: bool,
    pub(super) present: bool,
    pub(super) present_reason: Option<&'static str>,
    pub(super) view: bool,
}
pub(super) trait EditorAction: gpui::Action + Clone {
    const LABEL: &'static str;
    const ICON: Option<&'static str> = None;
    fn enabled(&self, available: EditorAvailability) -> bool;
    fn disabled_reason(&self, _: EditorAvailability) -> Option<&'static str> {
        None
    }
    fn execute(&self, app: &mut PsychoApp, window: &mut Window, cx: &mut Context<PsychoApp>);
}
macro_rules! editor_action {
    ($action:ty, $label:literal, $availability:ident, |$app:ident, $window:ident, $cx:ident| $execute:block $(, $icon:literal)? $(, reason = $reason:ident)?) => {
        impl EditorAction for $action {
            const LABEL: &'static str = $label;
            $(const ICON: Option<&'static str> = Some($icon);)?
            fn enabled(&self, available: EditorAvailability) -> bool { available.$availability }
            $(fn disabled_reason(&self, available: EditorAvailability) -> Option<&'static str> { available.$reason })?
            fn execute(&self, $app: &mut PsychoApp, $window: &mut Window, $cx: &mut Context<PsychoApp>) $execute
        }
    };
}
editor_action!(OpenFile, "開く", open, |app, window, cx| {
    app.open_picker(&MouseUpEvent::default(), window, cx);
});
editor_action!(SaveFile, "保存", save, |app, window, cx| {
    app.save(window, cx);
});
editor_action!(SaveAs, "別名保存", save_as, |app, window, cx| {
    app.prompt_save_as(window, cx);
});
editor_action!(
    UndoEdit,
    "元に戻す",
    undo,
    |app, window, cx| {
        app.undo_changes(window, cx);
    },
    "icons/undo.svg"
);
editor_action!(
    RedoEdit,
    "やり直す",
    redo,
    |app, window, cx| {
        app.redo_changes(window, cx);
    },
    "icons/redo.svg"
);
editor_action!(
    AddSlide,
    "スライド追加",
    edit,
    |app, window, cx| {
        app.add_slide(&MouseUpEvent::default(), window, cx);
    },
    "icons/add-slide.svg"
);
editor_action!(
    DeleteSlide,
    "スライドを削除",
    delete_slide,
    |app, window, cx| {
        app.delete_slide(&MouseUpEvent::default(), window, cx);
    }
);
editor_action!(
    MoveSlideEarlier,
    "前へ移動",
    earlier,
    |app, window, cx| {
        app.move_slide_selection(-1, window, cx);
    }
);
editor_action!(
    MoveSlideLater,
    "後ろへ移動",
    later,
    |app, window, cx| {
        app.move_slide_selection(1, window, cx);
    }
);
editor_action!(
    ShowInsertMenu,
    "挿入",
    edit,
    |app, window, cx| {
        app.open_insert_menu(InsertParent::Slide, window, cx);
    },
    "icons/insert.svg"
);
editor_action!(
    ShowViewMenu,
    "表示",
    view,
    |app, window, cx| {
        app.show_popup(Popup::View, window, cx);
    },
    "icons/view.svg"
);
editor_action!(
    ToggleNavigator,
    "Navigatorの表示切り替え",
    view,
    |app, window, cx| {
        app.toggle_pane(Pane::Navigator, window, cx);
    }
);
editor_action!(
    ToggleInspector,
    "Inspectorの表示切り替え",
    view,
    |app, window, cx| {
        app.toggle_pane(Pane::Inspector, window, cx);
    }
);
editor_action!(
    NarrowNavigator,
    "Navigatorを狭くする",
    view,
    |app, _window, cx| {
        app.change_pane_width(Pane::Navigator, -24., cx);
    }
);
editor_action!(
    WidenNavigator,
    "Navigatorを広くする",
    view,
    |app, _window, cx| {
        app.change_pane_width(Pane::Navigator, 24., cx);
    }
);
editor_action!(
    NarrowInspector,
    "Inspectorを狭くする",
    view,
    |app, _window, cx| {
        app.change_pane_width(Pane::Inspector, -24., cx);
    }
);
editor_action!(
    WidenInspector,
    "Inspectorを広くする",
    view,
    |app, _window, cx| {
        app.change_pane_width(Pane::Inspector, 24., cx);
    }
);
editor_action!(
    FitCanvas,
    "全体に合わせる",
    view,
    |app, _window, cx| {
        app.view.zoom = ZoomMode::Fit;
        cx.notify();
    }
);
editor_action!(ActualSize, "100%表示", view, |app, _window, cx| {
    app.view.zoom = ZoomMode::Actual;
    cx.notify();
});
editor_action!(
    PresentCurrent,
    "プレゼンテーション",
    present,
    |app, window, cx| {
        app.start_presentation(true, window, cx);
    },
    "icons/present.svg",
    reason = present_reason
);
editor_action!(
    PresentFirst,
    "最初からプレゼンテーション",
    present,
    |app, window, cx| {
        app.start_presentation(false, window, cx);
    },
    reason = present_reason
);
