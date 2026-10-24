# Open a presentation

Open a presentation lets a user launch PSYCHO with the default demo or a chosen KDL file and see the slide navigator and canvas preview.

## Sub-features

- `open-default` shows the built-in text of `examples/build-time.kdl` when no path argument is given. There is no document path yet, so the header reads 保存先未設定 until save-as.
- `open-path` opens a specific KDL file from the command line or File menu.
- `open-invalid` shows diagnostics instead of a partial preview when the file cannot be edited.

## How to get to it (user POV)

- Run `cargo run --features gui` with no arguments (built-in build-time deck, no file path).
- Run `cargo run --features gui -- path/to/presentation.kdl`.
- Choose **ファイル → 開く** (`Cmd-O`) and pick a `.kdl` file.

## Driving it with psycho-verify

Preconditions:

- Orca scripts need macOS. `psycho-verify launch` starts the scratch binary on macOS, and on Linux when `DISPLAY` or `WAYLAND_DISPLAY` is set.
- Scratch output directory does not exist yet.

- **Default demo (GUI).** Launch without args via acceptance fixture copy of `examples/build-time.kdl`. Run `psycho-verify launch --run-id <id> --kdl examples/build-time.kdl` then `Gui` snapshot. Slide list and canvas show the build-time deck title region.
- **CLI path (GUI).** `psycho-verify launch --run-id <id> --kdl validation/mvp-acceptance/layout.kdl` (or another fixture). Process stays up; `doctor` reports the fixture path.
- **Core load.** Run `psycho-verify drive-core --feature open-presentation`. `PresentationDocument::from_source` on valid KDL succeeds in unit tests (no GUI).
- **Proof.** Copy launch log and screenshot to `evidence/<run_id>/open-presentation.png` and record `feature=open-presentation` `entry=open-default`.

## Gotchas

- GUI acceptance refuses to overwrite an existing `--output` directory; always use a fresh `target/psycho-verify/<run_id>/` tree.
- Relative image paths resolve from the presentation file location; copy assets when moving fixtures.
- Invalid KDL still opens a window but editing and present actions stop until diagnostics are fixed.
- A no-argument launch does not open `examples/build-time.kdl` from disk. Save asks for a path.
