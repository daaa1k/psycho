"""Save an untitled draft, then continue the originally requested Open."""
import argparse
import json
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Next document" }; slide { heading "Next slide"; }; }\n', untitled=True)
records = []
try:
    gui.edit('CI のビルドが遅い', 48)
    gui.replace('Untitled draft retained')
    gui.press('開く')
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(gui.fixture), '--no-screenshot')
    gui.key('Return')
    gui.button('Open')
    state, _ = gui.snapshot('open-unsaved-modal')
    records.append(dict(name='open-confirmation', tree=state['snapshot']['treeText'], input=gui.state()))
    gui.button('保存')
    gui.button('退避先を選ぶ')
    archive = gui.out / 'archive'
    archive.mkdir()
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(archive), '--no-screenshot')
    gui.key('Return')
    gui.button('Save')
    saved = archive / 'presentation.kdl'
    assert b'heading "Untitled draft retained"' in saved.read_bytes()
    assert gui.state()['text'] == 'Next document', gui.state()
    assert gui.fixture.read_bytes() == gui.original
    state, _ = gui.snapshot('save-as-then-open')
    records.append(dict(name='continued-open', tree=state['snapshot']['treeText'], input=gui.state(), saved=saved.read_text()))
    print('PASS untitled draft Save As continues Open and preserves both documents')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
