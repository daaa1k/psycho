# Inspector element editing

The Inspector shows settings for the selected Element and a scrollable structure list. Canvas selection and the structure list share the selected Element.

## Sub-features

- Select every Element, including offscreen content and both Columns children.
- Edit heading, body, bullets, code, image reference, Caption, and column width.
- Keep unknown code languages until the user changes them.
- Reject invalid values before selection or pane changes.
- Preserve one input state when moving between Canvas and Inspector.
- Reach settings and structure rows with Tab, Shift-Tab, and arrow keys.
- Retain Inspector visibility and preferred width across an Overview roundtrip.

## How to get to it (user POV)

- Select an Element on Canvas or in the Inspector structure list.
- Scroll the structure list to reach later Elements.
- Double-click text on Canvas, then choose 全文をInspectorで編集.
- Select the code language or column width control for its related settings.
- Click the Canvas background to return to Presentation title settings.

## Driving it with psycho-verify

Preconditions:

- Edit mode with Inspector visible, a built macOS GUI executable, and Orca Accessibility access.
- A fresh output directory under target/psycho-verify.
- Run psycho-verify doctor before driving.

Run `python3 validation/mvp-acceptance/inspector_element_acceptance.py --output target/psycho-verify/<id>/inspector`.
The script checks all Element kinds, nested and offscreen selection, invalid width, pane changes, history, direct-edit glyph placement, Save, reload, and external conflict.
For Overview pane restoration, run `validation/overview/acceptance.py --section main` with the output and revision arguments from [the Overview guide](../../../../validation/overview/README.md).
For native Japanese composition, run the same script with `--ime` through `validation/mvp-acceptance/with_japanese_ime.swift`. The wrapper restores the previous input source and enablement.
Keep observations.json, saved.diff, PNG files, input state, and glyph trace with the run log. See `validation/inspector-element/README.md` for additional keyboard and VoiceOver evidence.

## Gotchas

- Fixed-count structure pagination is removed. Use Gui.outline_select and Gui.inspector_scroll in acceptance scripts.
- Canvas code uses a 22px font. Caption uses a 20px font.
- AX labels prove exposed controls. VoiceOver speech requires a separate native VoiceOver run.
- An invalid input blocks Inspector hiding even after focus moves to a button.
