import argparse
import json
import os
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--revision', required=True)
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to(root / 'target')
out.mkdir(parents=True, exist_ok=False)
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor.json')
source = 'presentation { metadata { title "Native input"; }; slide { heading "Original heading"; }; }'
expected = 'presentation { metadata { title "Native input"; }; slide { heading "Changed heading"; }; }'
gui = Gui(out, source)
records = []


def observe(name):
    gui.snapshot(name)
    editor = json.loads((out / 'editor.json').read_text())
    assert editor['revision'] == args.revision
    records.append(dict(name=name, input=gui.state(), revision=editor['revision']))
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))


try:
    gui.edit('Original heading', 48)
    observe('direct-edit-focused')
    assert gui.state()['focused']
    gui.replace('Changed heading')
    observe('selected-and-replaced')
    assert gui.state()['text'] == 'Changed heading'
    gui.menu('ファイル', '保存')
    assert gui.fixture.read_text() == expected
    assert not json.loads((out / 'editor.json').read_text())['dirty']
    (out / 'saved.kdl').write_text(gui.fixture.read_text())
    observe('saved')
    print('PASS native double click, select all, replacement and exact saved KDL', flush=True)
finally:
    gui.stop()
