# Open a presentation

Open a presentation lets a user launch PSYCHO with the default demo or a chosen KDL file and see the slide navigator and canvas preview.

## Sub-features

- `open-default` starts with `examples/build-time.kdl` when no path argument is given.
- `open-path` opens a specific KDL file from the command line or File menu.
- `open-invalid` shows diagnostics instead of a partial preview when the file cannot be edited.

## How to get to it (user POV)

- Run `cargo run --features gui` with no arguments (default demo).
- Run `cargo run --features gui -- path/to/presentation.kdl`.
- Choose **ファイル → 開く…** (`Cmd-O`) and pick a `.kdl` file.

## Driving it with psycho-verify

Preconditions:

- macOS with Orca for GUI paths; built `target/debug/psycho` for GUI.
- Scratch output directory does not exist yet.

- **Default demo (GUI).** Launch without args via acceptance fixture copy of `examples/build-time.kdl`. Run `psycho-verify launch --run-id <id> --kdl examples/build-time.kdl` then `Gui` snapshot. Slide list and canvas show the build-time deck title region.
- **CLI path (GUI).** `psycho-verify launch --run-id <id> --kdl validation/mvp-acceptance/layout.kdl` (or another fixture). Process stays up; `doctor` reports the fixture path.
- **Core load.** Run `psycho-verify drive-core --feature open-presentation`. `PresentationDocument::from_source` on valid KDL succeeds in unit tests (no GUI).
- **Proof.** Copy launch log and screenshot to `evidence/<run_id>/open-presentation.png` and record `feature=open-presentation` `entry=open-default`.

## Gotchas

- GUI acceptance refuses to overwrite an existing `--output` directory; always use a fresh `target/psycho-verify/<run_id>/` tree.
- Relative image paths resolve from the presentation file location; copy assets when moving fixtures.
- Invalid KDL still opens a window but editing and present actions stop until diagnostics are fixed.
