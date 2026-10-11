"""Verify popup Undo uses the same document history as its enabled state."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--expect-bug', action='store_true')
args = parser.parse_args()
os.environ['PSYCHO_EDITOR_STATE'] = str(args.output.resolve() / 'editor-state.json')
source = 'presentation { metadata { title "Popup history" }; slide { text "Original body"; }; }\n'
gui = Gui(args.output, source)
records = []


def observe(name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], gui.out / f'{name}.png')
    record = dict(name=name, editor=json.loads((gui.out / 'editor-state.json').read_text()),
                  input=gui.state(), tree=snapshot['snapshot']['treeText'])
    records.append(record)
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record


def click(label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) button ' + re.escape(label) + r'$', tree, re.MULTILINE)
    assert match, tree
    gui.act('click', '--element-index', match[1], '--no-screenshot')


try:
    for entry in ['toolbar', 'menu', 'shortcut']:
        gui.edit('Original body', 28)
        gui.replace('Edited body')
        click('表示')
        before = observe(entry + '-popup')
        assert before['editor']['undo'] and not before['input']['focused']
        if entry == 'toolbar':
            click('元に戻す')
        elif entry == 'menu':
            gui.menu('編集', '元に戻す')
        else:
            gui.hotkey('CmdOrCtrl+Z')
        after = observe(entry + '-undo')
        original = bytes.fromhex(after['editor']['sourceHex']).decode() == source
        assert original != args.expect_bug, after
        assert gui.fixture.read_bytes() == gui.original
        if args.expect_bug:
            break
    print('PASS reproduced popup history bug' if args.expect_bug else 'PASS popup Undo through toolbar, native menu and shortcut', flush=True)
finally:
    gui.stop()
