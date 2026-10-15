import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--exposure-only', action='store_true')
args = parser.parse_args()
source = 'presentation { metadata { title "Rows" }; slide { ' + ''.join('text "Row ' + str(i) + '"; ' for i in range(1, 24)) + '}; }\n'
gui = Gui(args.output, source)
records = []
helper = gui.out / 'native-identity'


def native():
    return json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))


def capture(name, label=None):
    state, _ = gui.snapshot(name)
    nodes = native()
    records.append(dict(name=name, tree=state['snapshot']['treeText'], input=gui.state(), native=nodes))
    if label:
        focus = next(n for n in nodes if n.get('AXFocused') and n['role'] != 'AXWindow')
        assert focus['label'] == label, focus
        assert '(selected) ' + label in state['snapshot']['treeText']
    print('PASS', name, flush=True)


def pointer(label):
    nodes = native()
    node = next(n for n in nodes if n['label'] == label)
    window = next(n for n in nodes if n['role'] == 'AXWindow')
    screenshot = gui.act('get-app-state')['screenshot']
    titlebar = window['size'][1] - screenshot['height']
    gui.click(node['position'][0] - window['position'][0] + 8, node['position'][1] - window['position'][1] - titlebar + 8)


try:
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_identity.swift')), '-o', str(helper)], check=True)
    if not args.exposure_only:
        pointer('Element 1 · 本文'); capture('pointer-pick-retains-row-focus', 'Element 1 · 本文')
        gui.key('Down'); capture('pointer-pick-arrow-selects-next', 'Element 2 · 本文')
        gui.key('Tab'); assert not gui.state()['focused']; capture('pointer-pick-tab-next-control')
        gui.outline_select('Element 3 · 本文'); capture('ax-pick-retains-row-focus', 'Element 3 · 本文')
        gui.key('Down'); capture('ax-pick-arrow-selects-next', 'Element 4 · 本文')
        gui.key('Return'); capture('return-retains-row-focus', 'Element 4 · 本文')
        gui.key('Up'); capture('return-arrow-selects-previous', 'Element 3 · 本文')
    gui.outline_select('Element 23 · 本文'); capture('last-row-selected', 'Element 23 · 本文')
    gui.key('Tab')
    for _ in range(10): gui.inspector_scroll(265, 'up')
    nodes = native()
    row = next(n for n in nodes if n['label'] == 'Element 23 · 本文')
    outline = next(n for n in nodes if n['label'] == 'スライドの構造一覧')
    assert row['position'][1] >= outline['position'][1] + outline['size'][1], (row, outline)
    capture('selected-last-row-scrolled-away')
    gui.hotkey('Shift+Tab')
    capture('shift-tab-immediately-reveals-selected-row', 'Element 23 · 本文')
    nodes = records[-1]['native']
    row = next(n for n in nodes if n['label'] == 'Element 23 · 本文')
    outline = next(n for n in nodes if n['label'] == 'スライドの構造一覧')
    assert outline['position'][1] <= row['position'][1]
    assert row['position'][1] + row['size'][1] <= outline['position'][1] + outline['size'][1], (row, outline)
    assert gui.save() == gui.original
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
