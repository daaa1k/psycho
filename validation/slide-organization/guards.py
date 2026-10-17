import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'validation/mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--revision', required=True)
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
records = []


def run(name, source, exercise):
    directory = out / name
    directory.mkdir()
    os.environ['PSYCHO_EDITOR_STATE'] = str(directory / 'editor-state.json')
    gui = Gui(directory, source)

    def state():
        return json.loads((directory / 'editor-state.json').read_text())

    def observe(label):
        snapshot = gui.act('get-app-state')
        shutil.copy(snapshot['screenshot']['path'], directory / f'{label}.png')
        value = dict(name=label, editor=state(), tree=snapshot['snapshot']['treeText'], input=gui.state())
        assert value['editor']['revision'] == args.revision
        records.append(value)
        (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
        return value

    def button(label):
        tree = gui.act('get-app-state')['snapshot']['treeText']
        index = re.search(r'^\s*(\d+) button ' + re.escape(label) + '$', tree, re.M).group(1)
        gui.act('click', '--element-index', index, '--no-screenshot')

    try:
        assert state()['revision'] == args.revision
        exercise(gui, state, observe, button)
    finally:
        gui.stop()


def empty(gui, state, observe, button):
    before = observe('empty')
    button('スライド追加')
    added = observe('added-first')
    assert bytes.fromhex(added['editor']['sourceHex']).decode().count('slide') == 1
    assert added['editor']['currentSlide'] == 0
    gui.menu('スライド', 'スライドを削除')
    deleted = observe('deleted-last')
    assert 'slide' not in bytes.fromhex(deleted['editor']['sourceHex']).decode()
    gui.menu('編集', '元に戻す')
    assert state()['sourceHex'] == added['editor']['sourceHex']
    gui.menu('編集', '元に戻す')
    assert state()['sourceHex'] == before['editor']['sourceHex']


def guards(gui, state, observe, button):
    gui.outline_select('Element 1 · 2列')
    button('左列幅を数値入力')
    gui.replace('0')
    invalid = observe('invalid-width')
    gui.key('Down')
    gui.key('Up')
    arrows = observe('input-arrow-priority')
    assert arrows['editor']['currentSlide'] == invalid['editor']['currentSlide']
    assert arrows['editor']['sourceHex'] == invalid['editor']['sourceHex']
    assert gui.state()['text'] == '0'
    gui.key('Backspace')
    assert gui.state()['text'] == ''
    gui.replace('0')
    invalid = observe('invalid-width-again')
    gui.menu('スライド', 'スライド追加')
    rejected = observe('invalid-add-rejected')
    assert rejected['editor']['sourceHex'] == invalid['editor']['sourceHex']
    assert rejected['editor']['undo'] == invalid['editor']['undo']
    assert gui.state()['text'] == '0'
    assert '列幅' in rejected['tree'] or '列幅' in gui.text('invalid-reason')
    helper = gui.out / 'pointer'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('pointer.swift')), '-o', str(helper)], check=True)
    def pointer(kind, x, y):
        subprocess.run([str(helper), str(gui.app.pid), kind, str(x), str(y)], check=True)
    for name, end, cancel in [('invalid-noop', (80, 180), False), ('invalid-escape', (80, 375), True), ('invalid-outside', (400, 375), False), ('invalid-drop', (80, 375), False)]:
        pointer('down', 80, 175)
        pointer('drag', 80, 375 if name != 'invalid-noop' else 180)
        if end[0] > 160:
            pointer('drag', *end)
        if cancel:
            gui.key('Escape')
        pointer('up', *end)
        result = observe(name)
        for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft', 'currentSlide']:
            assert result['editor'][field] == invalid['editor'][field], (name, field)
        assert gui.state()['text'] == '0'
    snapshot, rows = gui.snapshot()
    matches = [row for row in rows if row['text'] == '0'
               and row['x'] > snapshot['snapshot']['window']['width'] - 300]
    assert len(matches) == 1, rows
    gui.click(matches[0]['x'], matches[0]['y'])
    gui.replace('50')
    gui.menu('スライド', 'スライド追加')
    observe('repaired-add')
    button('Presentation title')
    snapshot, rows = gui.snapshot()
    matches = [row for row in rows if row['text'] == 'Guard test'
               and row['x'] > snapshot['snapshot']['window']['width'] - 300]
    assert len(matches) == 1, rows
    gui.click(matches[0]['x'], matches[0]['y'])
    gui.replace('unsaved draft')
    before = observe('pending-draft')
    gui.fixture.write_text(gui.fixture.read_text().replace('Guard test', 'External title'))
    time.sleep(1)
    conflict = observe('external-conflict')
    assert conflict['editor']['sourceHex'] == before['editor']['sourceHex']
    assert conflict['editor']['draft']
    tree = conflict['tree']
    assert 'button (disabled) スライド追加' in tree
    assert gui.state()['text'] == 'unsaved draft'
    gui.key('Backspace')
    observe('input-delete-priority')
    assert state()['sourceHex'] == before['editor']['sourceHex']


run('empty', 'presentation { metadata { title "Empty"; }; }', empty)
run('guards', 'presentation { metadata { title "Guard test"; }; slide { columns { column width=50 { text "Left"; }; column width=50 { text "Right"; }; }; }; slide { heading "Second"; }; }', guards)
(out / 'result.json').write_text(json.dumps(dict(result='PASS', revision=args.revision, macOS=subprocess.check_output(['sw_vers'], text=True)), indent=2))
