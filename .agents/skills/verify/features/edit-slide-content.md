# Edit slide content

Edit slide content lets a user select a slide, change title/body/elements on the canvas or Inspector, and undo or redo those document changes.

## Sub-features

- `select-slide` changes the preview via the slide list.
- `direct-edit` double-clicks supported element fields on the canvas.
- `undo-redo` reverses document edits via toolbar, Edit menu, or `Cmd-Z` / `Cmd-Shift-Z`.

## How to get to it (user POV)

- Click a slide in the left navigator.
- Double-click heading, body, bullets, code, or caption text on the canvas.
- Use **編集 → 元に戻す** / **やり直す** or `Cmd-Z` / `Cmd-Shift-Z` when focus is not in a single-line field.

## Driving it with psycho-verify

Preconditions:

- Healthy scratch `psycho` with `examples/build-time.kdl` (or copy) loaded.
- `psycho-verify doctor --run-id <id>` is `ok`.

- **Sample workflow (GUI).** Run `python3 validation/mvp-acceptance/sample_workflow_acceptance.py --output target/psycho-verify/<id>/sample-workflow`. Script edits title, heading, body, code, columns, image path, caption, and structure; asserts Save/Undo/Redo byte equality per step.
- **Proof.** Keep `sample-workflow/sample-*.png` and `result.txt` under `evidence/<run_id>/edit-slide-content/`; stdout must show `PASS` lines for each named edit.

## Gotchas

- While a text field has focus, `Cmd-Z` may target field history before document history.
- Escape commits in-progress canvas edits before other actions.
- External file changes can block undo/redo until conflict is resolved.
