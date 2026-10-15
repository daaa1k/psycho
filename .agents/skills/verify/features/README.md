# PSYCHO verification map

This directory is the maintained source for verifying user-facing PSYCHO behavior.
Read this index before driving the app, then open the matching feature file.

## Baseline preconditions

- **macOS GUI drives:** One connected display; Orca with Accessibility and screen recording;
  Xcode CLI tools; `cargo build --locked --features gui --bin psycho`; no other `psycho`
  processes owned by the acceptance scripts.
- **Scratch data:** Every GUI script uses a **new** directory under `target/` (see
  `validation/mvp-acceptance/gui_session.py`). Never point acceptance at a user's document.
- **Core drives:** `cargo test --locked` without starting `psycho` (default features).
- Run `psycho-verify doctor` and require `map-lint` clean before the first GUI drive.

## Driving conventions

- Start from the baseline unless a feature file says otherwise.
- GUI: drive through Orca (`Gui.act`) using stable labels from the UI and README; copy
  exact Japanese toolbar strings from recipes.
- Core: use `psycho-verify drive-core --feature <id>` or the explicit `cargo test` filter
  named in the feature file.
- Restore mutated fixtures in GUI scripts; do not delete proof artifacts during cleanup.

## Proof and skip reporting

- Capture the action and resulting state, not only the final frame.
- GUI proof: PNG (and script JSON when present) under `evidence/<run_id>/`.
- Core proof: `cargo test` output and on-disk KDL bytes when the scenario writes files.
- Record feature id and user entry point with every artifact.
- Report unreachable paths with the attempted route and missing prerequisite (OS, Orca, IME).
- Do not mark an entry point verified if only a different path was exercised.

## Feature entry contract

Each feature file has an H1 title, one introductory paragraph, then exactly four H2 sections
in this order: `Sub-features`, `How to get to it (user POV)`, `Driving it with <harness>`,
`Gotchas`. The driving section starts with `Preconditions:`.

## Features

- [Open a presentation](./open-presentation.md) — launch, default demo, open KDL from disk.
- [Edit slide content](./edit-slide-content.md) — select slides, direct edit, Undo/Redo.
- [Save document](./save-document.md) — persist edits and preserve KDL formatting rules.
- [Present slideshow](./present-slideshow.md) — start presentation from toolbar or menu.
- [Diagnostic navigation](./diagnostic-navigation.md) — syntax/schema errors and jump-to-source.

- [Inspector element editing](./inspector-element.md) covers element settings, structure scrolling, and input transfer.
