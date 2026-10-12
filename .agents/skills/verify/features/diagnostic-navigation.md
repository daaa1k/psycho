# Diagnostic navigation

Diagnostic navigation lets a user see KDL syntax/schema problems (and related asset issues) and jump from a diagnostic to the affected source or slide element.

## Sub-features

- `syntax-error` shows parse errors with file, line, column, and source line.
- `schema-error` shows Presentation Schema v0.1 violations.
- `jump-to-source` selects readonly source or the target slide/element from the diagnostic list.

## How to get to it (user POV)

- Open a `.kdl` file that fails to parse or validate.
- Click a diagnostic row in the UI to navigate to the problem.

## Driving it with psycho-verify

Preconditions:

- Fixture KDL with intentional syntax or schema errors under `validation/mvp-acceptance/`.
- Scratch `psycho` launched on that fixture.

- **Navigation suite (GUI).** `python3 validation/mvp-acceptance/diagnostic_navigation_acceptance.py --output target/psycho-verify/<id>/diagnostics`
- **Proof.** `observations.json` in the output dir copied to `evidence/<run_id>/diagnostic-navigation/`; entries must include marker hits such as `PSYCHO_READONLY_NEGATIVE_*` for readonly source jumps.

## Gotchas

- While external conflict is active, navigation and editing may be limited until reload.
- Image diagnostics jump to elements, not readonly KDL lines.
- Invalid initial file shows diagnostics without partial slide preview.
