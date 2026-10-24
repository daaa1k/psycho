---
name: verify
description: >
  Drive PSYCHO like a user: macOS GPUI editor, fullscreen presentation, and KDL
  document lifecycle. Use to prove UI/acceptance behavior, refresh the feature
  map, or run the weekly verification-skill maintenance pass on this repo.
---

# Verify PSYCHO

PSYCHO is a macOS presentation editor (GPUI) with a Rust core library. Users edit
KDL on disk, preview slides, present fullscreen, and fix diagnostics. Agents prove
behavior on the **real macOS GUI** via Orca plus the acceptance harness under
`validation/mvp-acceptance/`. On Linux and Windows CI, prove the **document core**
with `cargo test` and keep the feature map structurally honest.

Put the helper on `PATH` before driving:

```sh
export PATH="$PWD/.agents/skills/verify/helpers:$PATH"
psycho-verify --help
```

Every subcommand prints a one-line JSON summary on success and exits non-zero on failure.

## Launch

**macOS GUI (primary surface)**

1. Build once: `cargo build --locked --features gui --bin psycho`
2. Each drive uses a **fresh scratch directory** under `target/psycho-verify/<run_id>/`
   (never reuse an existing fixture path).
3. Start the app through acceptance `Gui` (see `validation/mvp-acceptance/gui_session.py`)
   or run `psycho-verify launch --run-id <id> --kdl <path>` which builds if needed and
   records PID, fixture path, and log under the scratch dir.
4. Ready when Orca lists the scratch `psycho` PID and `psycho-verify doctor --run-id <id>` reports `ok`.
5. Teardown: `psycho-verify cleanup --run-id <id>` (terminates only that PID). Do not `pkill psycho`.

**Linux GUI**

`psycho-verify launch` starts the same scratch process when `DISPLAY` or `WAYLAND_DISPLAY` is set. `doctor --run-id` then checks that the recorded PID is alive (`gui` is `pid_alive`). Orca acceptance scripts stay on macOS. Prove the window with a screenshot. The `cmd-*` bindings use the Super key (save is Super+S). Teardown is the same `cleanup` command.

**Core library (headless, all platforms)**

No long-lived process. For a single core recipe:

```sh
psycho-verify drive-core --feature save-document --run-id <id>
```

## Doctor

Read-only. Run before the first drive, after any failed drive, and whenever state looks wrong.

```sh
psycho-verify doctor --run-id <id>   # macOS: scratch PID + Orca; all OS: map + toolchain
```

Requires: feature map passes `psycho-verify map-lint`; `cargo` on `PATH`; on macOS drives,
`orca` and a built `target/debug/psycho`. Doctor never mutates documents or kills processes.

## Drive

**macOS GUI** — follow the feature file under `features/` literally. Recipes use
`orca computer` via `Gui.act` (see `gui_session.py`). Prefer toolbar/menu labels and
OCR-visible Japanese strings from `examples/build-time.kdl`, not pixel coordinates alone.

Example (full recipe in [save-document](features/save-document.md)):

```sh
export PSYCHO_VERIFY_RUN_ID=demo-1
psycho-verify launch --run-id "$PSYCHO_VERIFY_RUN_ID" --kdl examples/build-time.kdl
psycho-verify doctor --run-id "$PSYCHO_VERIFY_RUN_ID"
python3 validation/mvp-acceptance/sample_workflow_acceptance.py \
  --output "target/psycho-verify/$PSYCHO_VERIFY_RUN_ID/sample-workflow"
psycho-verify cleanup --run-id "$PSYCHO_VERIFY_RUN_ID"
```

**Core** — `psycho-verify drive-core --feature <id>` runs the mapped `cargo test` filter
documented in that feature file (no GUI).

## Evidence

Store proof under `.agents/skills/verify/evidence/<run_id>/`. Capture the user action
and resulting state: acceptance PNG/JSON under the script `--output` dir **and** copy
or reference them into the evidence dir. GUI proof needs a screenshot with slide chrome
visible; core proof needs `cargo test` stdout and the saved KDL bytes when applicable.
Cleanup removes scratch processes and dirs under `target/psycho-verify/<run_id>/` but
**never** deletes `evidence/<run_id>/`.

## Cleanup

```sh
psycho-verify cleanup --run-id <id>
```

Stops the launched PID for `<id>` only, removes `target/psycho-verify/<id>/` scratch state,
and leaves evidence untouched. After cleanup, confirm artifacts still exist under
`.agents/skills/verify/evidence/<run_id>/`.

## Helpers

| Command | Purpose |
| --- | --- |
| `psycho-verify map-lint` | Feature map index hygiene (required H2s, links). |
| `psycho-verify doctor` | Preconditions for driving. |
| `psycho-verify launch` | Scratch `psycho` instance (macOS, or Linux with a display). |
| `psycho-verify drive-core --feature <id>` | Headless cargo test for one map entry. |
| `psycho-verify maintain-verification-skill` | Weekly/CI pass: `map-lint` + core tests (+ macOS extras when available). |
| `psycho-verify cleanup` | Tear down one run. |

Script: [helpers/psycho-verify](helpers/psycho-verify) (must stay executable).

## Feature map

Maintained recipes: [features/README.md](features/README.md). A proof that only hits
one convenient path is incomplete when the map lists other user entry points for the
same feature.

## Maintenance

Run `/maintain-verification-skill` after user-facing surface changes. CI runs
`psycho-verify maintain-verification-skill` weekly (see `.github/workflows/verify-skill-maintenance.yml`).
