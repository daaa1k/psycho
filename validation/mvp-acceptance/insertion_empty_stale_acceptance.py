import argparse
import json
import os
import re
import subprocess
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--stale-only', action='store_true')
args = parser.parse_args()
records = []

def ax(gui, label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) (?:button|radio button) (?:\([^)]*\) )?' + re.escape(label) + r'(?: ✓)?(?:, (?:Secondary Actions:|Value:).*)?$', tree, re.M)
    assert match, (label, tree)
    gui.act('click', '--element-index', match[1], '--no-screenshot')

def checkpoint(gui, name):
    state, _ = gui.snapshot(name)
    records.append({'name': name, 'tree': state['snapshot']['treeText'], 'source': gui.fixture.read_text()})
    print('PASS', name, flush=True)

for entry in (() if args.stale_only else ('toolbar', 'native')):
    gui = Gui(args.output / entry, 'presentation {\n metadata { title "Empty" }\n}\n')
    try:
        if entry == 'toolbar':
            ax(gui, '挿入')
            checkpoint(gui, 'empty-create-route')
            gui.key('Return')
        else:
            native = gui.out / 'native-menu-path'
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_menu_path.swift')), '-o', str(native)], check=True)
            gui.act('get-app-state', '--restore-window')
            subprocess.run([str(native), str(gui.app.pid), '挿入', 'スライド追加'], check=True)
            gui.act('get-app-state')
        assert gui.save().count(b'slide {') == 1, gui.fixture.read_bytes()
        checkpoint(gui, 'empty-created-' + entry)
    finally:
        gui.stop()

source = 'presentation {\n metadata { title "Stale" }\n slide {\n columns { column width=50; column width=50 } // A\n columns { column width=50; column width=50 } // B\n }\n}\n'
os.environ['PSYCHO_EDITOR_STATE'] = str((args.output / 'stale' / 'editor.json').resolve())
gui = Gui(args.output / 'stale', source)
try:
    ax(gui, '挿入');ax(gui, '2列 1 の左列')
    checkpoint(gui, 'old-parent-selected')
    changed = source.replace(' columns { column width=50; column width=50 } // A\n', '')
    gui.fixture.write_text(changed)
    time.sleep(1.5)
    tree = gui.act('get-app-state')['snapshot']['treeText']
    assert re.search(r'button \(disabled\) 本文', tree), tree
    checkpoint(gui, 'old-parent-disabled-after-reload')
    press = gui.out / 'stale-press'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('insertion_stale_press.swift')), '-o', str(press)], check=True)
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(press), str(gui.app.pid)], check=True)
    state, rows = gui.snapshot('old-request-executed-and-refused')
    assert any('挿入メニューを開き直してください' in row['text'] and row['y'] > state['snapshot']['window']['height'] - 65 for row in rows), rows
    checkpoint(gui, 'old-request-executed-and-refused')
    gui.key('Escape')
    assert gui.save() == changed.encode(), gui.fixture.read_bytes()
    checkpoint(gui, 'old-parent-no-insertion')
finally:
    gui.stop()
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
