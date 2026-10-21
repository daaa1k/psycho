"""Exercise the editor shell through its owned macOS window and saved KDL."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--appearance', choices=['light', 'dark'], required=True)
parser.add_argument('--revision', default=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip())
args = parser.parse_args()
args.output = args.output.resolve()
os.environ['PSYCHO_EDITOR_STATE'] = str(args.output / 'editor-state.json')
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = args.appearance
source = 'presentation { metadata { title "Editor shell" }; slide id="first" { heading "Heading"; text "Original body"; }; slide id="second" { heading "Second slide"; }; }\n'
gui = Gui(args.output, source)
records = []


def state():
    return json.loads((args.output / 'editor-state.json').read_text())


def observe(name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], gui.out / f'{name}.png')
    record = dict(name=name, editor=state(), input=gui.state(),
                  tree=snapshot['snapshot']['treeText'], window=snapshot['snapshot']['window'],
                  savedSha256=hashlib.sha256(gui.fixture.read_bytes()).hexdigest())
    records.append(record)
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record


def button(label):
    snapshot = gui.act('get-app-state')
    row = next(re.finditer(r'^\s*(\d+) button ' + re.escape(label) + r'$',
                          snapshot['snapshot']['treeText'], re.MULTILINE), None)
    assert row, (label, snapshot['snapshot']['treeText'])
    gui.act('click', '--element-index', row.group(1), '--no-screenshot')


def invariant(before):
    after = state()
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft']:
        assert after[field] == before[field], (field, before, after)
    assert gui.fixture.read_bytes() == gui.original


try:
    helper = gui.out / 'resize-window'
    subprocess.run(['xcrun', 'swiftc', str(root / 'validation/mvp-acceptance/resize_window.swift'), '-o', str(helper)], check=True)
    before = state()
    assert before['revision'] == args.revision, before
    assert before['dark'] == (args.appearance == 'dark'), before
    for width, height in [(1024, 700), (1280, 860), (1440, 960)]:
        subprocess.run([str(helper), str(gui.app.pid), str(width), str(height + 32)], check=True)
        time.sleep(.4)
        record = observe(f'{args.appearance}-{width}x{height}-fit')
        tree = record['tree']
        for label in ['スライド追加', '挿入', '表示', 'プレゼンテーション', '発表メニュー']:
            assert f'button {label}' in tree, tree
        editor = record['editor']
        assert abs(editor['canvasWidth'] - (width - editor['navigatorWidth'] - editor['inspectorWidth'] - 32)) < 1, record
        assert abs(editor['scale'] - min(editor['canvasWidth'] / 1280, editor['canvasHeight'] / 720)) < .0001, editor
        invariant(before)
    for action, field in [('Navigatorを隠す', 'navigator'), ('Inspectorを隠す', 'inspector')]:
        gui.menu('表示', action)
        record = observe('hidden-' + field)
        assert record['editor'][field] is False
        invariant(before)
    gui.menu('表示', '100%表示')
    assert state()['actual'] and state()['scale'] == 1
    observe('actual-both-hidden')
    gui.menu('表示', 'Navigatorを表示')
    gui.menu('表示', 'Inspectorを表示')
    observe('actual-both-shown')
    gui.act('scroll', '--x', '600', '--y', '550', '--direction', 'right', '--no-screenshot')
    gui.act('scroll', '--x', '600', '--y', '550', '--direction', 'down', '--no-screenshot')
    observe('actual-scrolled')
    invariant(before)
    gui.menu('表示', '全体に合わせる')
    gui.menu('表示', 'Navigatorを広くする')
    assert state()['navigatorWidth'] > before['navigatorWidth']
    gui.menu('表示', 'Inspectorを狭くする')
    assert state()['inspectorWidth'] < before['inspectorWidth']
    observe('pane-width-menu')
    invariant(before)
    button('表示')
    observe('view-popup')
    gui.key('Escape')
    assert 'container 表示' not in gui.act('get-app-state')['snapshot']['treeText']
    observe('popup-return')
    gui.key('Tab')
    observe('tab-first-control')
    gui.key('Tab')
    observe('tab-second-control')
    gui.edit('Original body', 28)
    gui.replace('Edited body')
    assert gui.state()['focused']
    gui.press('Undo')
    assert gui.state()['text'] == 'Original body'
    gui.press('Redo')
    assert gui.state()['text'] == 'Edited body'
    observe('field-undo-redo')
    edited = gui.save()
    assert b'text "Edited body"' in edited
    observe('saved')
    gui.press('Undo')
    assert gui.fixture.read_bytes() == edited
    assert gui.save() == gui.original
    gui.press('Redo')
    assert gui.save() == edited
    observe('document-undo-redo')
    button('プレゼンテーション')
    assert state()['presenting']
    observe('presentation')
    gui.key('Escape')
    time.sleep(1)
    assert not state()['presenting']
    assert gui.fixture.read_bytes() == edited
    observe('presentation-return')
    (gui.out / 'run.json').write_text(json.dumps(dict(
        revision=before['revision'], binarySha256=hashlib.sha256((root / 'target/debug/psycho').read_bytes()).hexdigest(),
        os=subprocess.check_output(['sw_vers'], text=True), appearance=args.appearance,
        original=gui.original.decode(), saved=edited.decode()), ensure_ascii=False, indent=2))
    print('PASS editor shell sizes, panes, Fit, 100%, scroll, field/document history, Save and presentation', flush=True)
except Exception:
    observe('failure')
    raise
finally:
    gui.stop()
