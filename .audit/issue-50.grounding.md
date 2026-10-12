# Navigator grounding

Issue #50 and parent #48 govern the contract. Source base is 343cc05.

main.rs render_slide_list:2923 currently renders every slide as ListItem plus button. No thumbnail, tooltip, keyboard context or scroll tracking. render_canvas:4140 shares editor and presentation placement at 1280x720, margins 64x48 and element gap 24. Extract white slide body with explicit scale and read-only/editor mode. render_canvas_element:4239 owns Columns and suppresses editing when editing=false; render_element alone cannot render Columns. text.rs shared_text:54 shapes at base size and scales glyph positions without minimum sizes.

PresentationDocument owns source/model/history/external state. PsychoApp owns current_slide, target, editor draft, asset_images. render_editor passes preview_model to Navigator and canvas. preview_model renders valid drafts without changing source/history. commit_draft validates, commits input history and refreshes assets. Undo/Redo, did_reload, refresh_assets update current model/images synchronously. Initial invalid model is None. External invalid state retains last good model plus mismatch footer. Asset changes are polled and refreshed. No async thumbnail cache means no stale result race.

Navigator selection must reuse commit_draft, preserve draft during blocked external state, clear canvas editing normally, and focus Navigator instead of select_title root return. Dedicated navigator FocusHandle and context prevent stealing TextInput arrows/enter. Stable row identity separate from label; selected row uses aria_selected and active descendant. GPUI pinned rev has native AccessKit mac adapter and action registration. Actual speech remains unverified in #49.

GPUI uniform_list renders visible range only and UniformListScrollHandle.scroll_to_item(Nearest) exposes selected row. Uniform fixed row height can vary with shared navigator width. Existing validation/mvp-acceptance/gui_session.py drives scratch PID with Orca and glyph traces. editor-shell/scroll.py sends native scroll and compares real origins. editor_validation.rs exports source/history/pane state but needs selected slide, navigator focus and visible range observations.

Grounding gathered by gpt-6.1-sol readonly how explainer and independently opened source references above. No runtime verification yet.
