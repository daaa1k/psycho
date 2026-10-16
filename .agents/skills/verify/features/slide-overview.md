# Slide Overview

Overview displays the Presentation as responsive thumbnails and returns the selected Slide to Edit mode. Navigator and Overview share content and document operations while retaining separate focus and scroll positions.

## Sub-features

- Switch modes from the native View menu and toolbar popup.
- Retain selection, document history, pane preferences, and pending input validation.
- Navigate the grid, reorder, add, delete, Undo, and Redo through the existing Slide actions.
- Reach the final Slide in a 100-Slide Presentation at three window sizes in both appearances.
- Activate a Slide with VoiceOver and confirm Japanese composition before switching.
- Edit, save, present, and return to the same Slide.

## How to get to it (user POV)

- Choose 表示, then Overview in the native menu or toolbar popup.
- Click a thumbnail or use the arrow keys to select a Slide.
- Double-click a thumbnail or press Return to return to Edit mode.
- Use the Slide menu, context menu, or keyboard commands to change the order.

## Driving it with psycho-verify

Preconditions:

- A built macOS GUI executable, Orca Accessibility access, and one active GUI driver.
- A fresh scratch output directory under target/.
- Run psycho-verify doctor before driving.

Run the commands in [the Overview acceptance guide](../../../../validation/overview/README.md).
The main, drag, guards, empty, invalid, representative, and light/dark layout sections use native events and compare the resulting source and state.
Run the separate VoiceOver and Japanese IME recipes for native assistive technology and composition proof.
Keep representative screenshots, result JSON, saved KDL, and restoration records with the build revision.

## Gotchas

- Inspector is effectively hidden in Overview. Its saved visibility and width remain unchanged.
- An invalid draft or an externally frozen pending draft refuses mode switching before applying the draft.
- Grid rows are virtualized. Arrow navigation exposes the selected Slide.
- AX labels do not prove VoiceOver speech or activation.
