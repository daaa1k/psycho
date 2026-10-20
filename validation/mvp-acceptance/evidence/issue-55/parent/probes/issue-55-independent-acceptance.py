import json
import os
import re
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'validation/mvp-acceptance'))
from gui_session import Gui

out = root / 'target' / sys.argv[1]
records = []

def ax(gui, name):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) button ' + re.escape(name) + r'$', tree, re.M)
    assert match, (name, tree)
    gui.act('click', '--element-index', match[1], '--no-screenshot')

def observe(gui, name):
    state, rows = gui.snapshot(name)
    record = dict(name=name, tree=state['snapshot']['treeText'],
                  text=''.join(row['text'] for row in rows), input=gui.state(),
                  disk=gui.fixture.read_bytes().hex())
    records.append(record)
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record

source = 'presentation { metadata { title "Invalid draft"; }; slide { heading "Keep source"; }; }\n'
gui = Gui(out / 'invalid-draft', source)
try:
    ax(gui, 'Presentation title')
    tree = gui.act('get-app-state')['snapshot']['treeText']
    index = re.search(r'^\s*(\d+) text field .*Presentation のタイトル', tree, re.M)[1]
    gui.act('click', '--element-index', index, '--no-screenshot')
    gui.replace('')
    assert gui.state()['text'] == ''
    gui.hotkey('CmdOrCtrl+W')
    modal = observe(gui, 'invalid-close-modal')
    assert all('button ' + name in modal['tree'] for name in ('保存', '破棄', 'キャンセル'))
    gui.key('Escape')
    assert gui.state()['text'] == '' and gui.state()['focused']
    assert gui.fixture.read_bytes() == gui.original
    gui.hotkey('CmdOrCtrl+W')
    ax(gui, '保存')
    failed = observe(gui, 'invalid-save-refusal')
    assert 'button 再試行' in failed['tree']
    assert gui.fixture.read_bytes() == gui.original
    gui.key('Escape')
    assert gui.state()['text'] == '' and gui.state()['focused']
    gui.replace('Recovered title')
    gui.press('保存')
    assert b'title "Recovered title"' in gui.fixture.read_bytes()
    observe(gui, 'invalid-repaired-and-saved')
    print('PASS invalid draft close/cancel, save rejection, input focus, repair/save', flush=True)
finally:
    gui.stop()

source = 'presentation { metadata { title "Noncurrent image"; }; slide { heading "First"; }; slide { image "missing-image.png"; }; }\n'
gui = Gui(out / 'noncurrent-image', source)
try:
    tree = gui.act('get-app-state')['snapshot']['treeText']
    assert re.search(r'button \(disabled\) プレゼンテーション', tree), tree
    label = re.search(r'button (診断 [1-9]\d*件)', tree)[1]
    ax(gui, label)
    before = observe(gui, 'image-diagnostics')
    rows = [line for line in before['tree'].splitlines() if 'button' in line and '画像' in line and 'Slide 2' in line]
    assert len(rows) == 1, before['tree']
    gui.act('click', '--element-index', rows[0].strip().split()[0], '--no-screenshot')
    selected = observe(gui, 'image-target')
    assert selected['input']['text'] == 'missing-image.png'
    assert gui.fixture.read_bytes() == gui.original
    print('PASS missing image on noncurrent slide blocks presentation and diagnostic selects target', flush=True)
finally:
    gui.stop()
print('PASS independent acceptance', flush=True)
