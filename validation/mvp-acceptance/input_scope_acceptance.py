"""Undo controls must stay inside a focused field's editing session."""
import argparse
from pathlib import Path
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root/'target/mvp-input-scope')
gui = Gui(parser.parse_args().output,
    'presentation { metadata { title "Input scope" }; slide id="one" {}; }\n')
try:
    gui.press('本文を追加')
    assert gui.state()['focused'] and gui.state()['undo'] == 0
    gui.press('Undo')
    gui.snapshot('focused-empty-undo')
    # Leaving an empty input must still leave the previously added Element.
    gui.key('Escape')
    added = gui.save()
    assert b'text ""' in added, 'Focused Undo must not undo the Element addition'
    gui.press('Undo')
    assert gui.save() == gui.original
    gui.press('本文を追加')
    gui.replace('New text')
    gui.press('Undo')
    assert gui.state()['text'] == '' and gui.state()['undo'] == 0
    gui.press('Redo')
    assert gui.state()['text'] == 'New text'
    gui.press('Undo')
    gui.press('Undo')
    assert gui.state()['text'] == ''
    gui.press('Redo')
    saved = gui.save()
    assert b'text "New text"' in saved
    # Save commits and ends editing; document history remains available.
    gui.hotkey('CmdOrCtrl+Z')
    assert gui.fixture.read_bytes() == saved, 'Undo must not write to disk'
    assert b'text ""' in gui.save()
    gui.hotkey('CmdOrCtrl+Shift+Z')
    assert gui.save() == saved
    gui.snapshot('saved-history-retained')
    print('PASS focused toolbar Undo/Redo boundary and history after Save', flush=True)
finally:
    gui.stop()
