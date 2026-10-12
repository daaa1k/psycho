# Present slideshow

Present slideshow lets a user enter fullscreen slide show from the current or first slide and return to editing with Escape.

## Sub-features

- `present-from-start` begins at the first slide (**プレゼンテーション → 最初からプレゼンテーション**).
- `present-from-current` begins at the selected slide (toolbar **プレゼンテーション** or `Cmd-Option-P`).
- `present-keys` advances with Right/Down/Space/PageDown and goes back with Left/Up/PageUp.

## How to get to it (user POV)

- Click **プレゼンテーション** in the toolbar (current slide).
- Choose **プレゼンテーション → 最初からプレゼンテーション** or **現在からプレゼンテーション** from the menu.
- Press `Cmd-Option-P` for present from current.

## Driving it with psycho-verify

Preconditions:

- Document has no blocking diagnostics (assets valid, schema clean).
- Single display; acceptance sets fullscreen width via `PSYCHO_ACCEPTANCE_FULLSCREEN_WIDTH` when needed.

- **Keys and snapshots (GUI).** `python3 validation/mvp-acceptance/presentation_workflow_acceptance.py --section keys --output target/psycho-verify/<id>/present-keys`
- **Full present flow.** `fullscreen_acceptance.py --output target/psycho-verify/<id>/fullscreen` for repeated enter/exit cycles.
- **Proof.** Store `fullscreen-checks.csv` or section PNGs under `evidence/<run_id>/present-slideshow/`; window dimensions must match acceptance expectations.

## Gotchas

- Present commits pending edits but does not auto-save to disk.
- Asset or layout diagnostics prevent starting present until fixed.
- During present, external file edits apply after exiting fullscreen.
