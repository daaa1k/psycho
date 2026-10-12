# Save document

Save document lets a user write edits back to KDL on disk, preserving unedited formatting, comments, and raw strings.

## Sub-features

- `save` overwrites the current file with `Cmd-S` or **ファイル → 保存**.
- `save-as` writes to a new path via **別名で保存…** (`Cmd-Shift-S`).
- `save-core` persists through `PresentationDocument::save` without the GUI.

## How to get to it (user POV)

- Press `Cmd-S` or choose **ファイル → 保存**.
- Press `Cmd-Shift-S` or choose **ファイル → 別名で保存…**.

## Driving it with psycho-verify

Preconditions:

- Document is dirty after an edit, or core test uses a temp directory.
- Save destination is writable; acceptance uses disposable paths under `target/`.

- **GUI save path.** `sample_workflow_acceptance.py` ends with save/reload checks; or `no_edit_save_acceptance.py` for formatting preservation.
- **Core save race checks.** Run `psycho-verify drive-core --feature save-document` which executes:
  `cargo test --locked --lib save_ -- --nocapture` (matches both save regression tests).
- **Proof.** For core: capture test stdout to `evidence/<run_id>/save-document-cargo-test.log`. For GUI: saved KDL bytes hash in script JSON.

## Gotchas

- Save replaces only edited values in source; assert full bytes when testing formatting preservation.
- Save-as to another folder rewrites relative image paths and clears undo history on success.
- Conflict or external change blocks save until resolved.
