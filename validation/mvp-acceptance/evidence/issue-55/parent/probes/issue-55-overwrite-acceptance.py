import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'validation/mvp-acceptance'))
from gui_session import Gui
out = root / 'target' / sys.argv[1]
gui = Gui(out, 'presentation { metadata { title "Overwrite confirmation"; }; slide { heading "Original"; }; }\n')
archive = gui.out / 'archive'
archive.mkdir()
dest = archive / 'presentation.kdl'
existing = b'presentation { metadata { title "Existing destination"; }; slide { heading "Keep until confirmed"; }; }\n'
dest.write_bytes(existing)
records = []

def choose():
    gui.press('別名保存')
    gui.button('退避先を選ぶ')
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(archive), '--no-screenshot')
    gui.key('Return')
    gui.button('Save')
    state, rows = gui.snapshot('replace-confirmation-' + str(len(records)))
    tree = state['snapshot']['treeText']
    assert 'button Replace' in tree and 'button Cancel' in tree, tree
    assert dest.read_bytes() == existing
    assert gui.fixture.read_bytes() == gui.original
    records.append(dict(name='confirmation', tree=tree, destination=dest.read_bytes().hex()))

try:
    gui.edit('Original', 48)
    gui.replace('Retained overwrite draft')
    choose()
    gui.button('Cancel')
    gui.key('Escape')
    assert dest.read_bytes() == existing
    assert gui.fixture.read_bytes() == gui.original
    assert gui.state()['text'] == 'Retained overwrite draft'
    gui.snapshot('replace-cancel-preserves-files')
    choose()
    gui.button('Replace')
    gui.snapshot('replace-confirmed')
    assert dest.read_bytes() == gui.original.replace(b'Original', b'Retained overwrite draft')
    assert gui.fixture.read_bytes() == gui.original
    saved = dest.read_bytes()
    gui.hotkey('CmdOrCtrl+Z')
    gui.press('保存')
    assert dest.read_bytes() == saved
    records.append(dict(name='confirmed', destination=dest.read_bytes().hex(), original=gui.fixture.read_bytes().hex()))
    print('PASS native overwrite confirmation cancel/replace, both originals protected until consent, saved draft and history clear', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
