import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
os.environ['PSYCHO_IME_RECTS'] = str(args.output.resolve() / 'character-rects.csv')
gui = Gui(args.output, 'presentation { metadata { title "IME keys" }; slide { heading "Native IME"; text "Body"; }; }\n')
records = []


def record(name):
    state, _ = gui.snapshot(name)
    windows = json.loads(subprocess.check_output(['xcrun', 'swift', str(root / 'validation/mvp-acceptance/ime_windows.swift'), str(gui.app.pid)], text=True))
    panels = [w for w in windows if w['layer'] == 20]
    for index, panel in enumerate(panels):
        subprocess.run(['screencapture', '-x', '-l', str(panel['id']), str(gui.out / (name + '-candidate-' + str(index) + '.png'))], check=True)
    records.append(dict(name=name, input=gui.state(), tree=state['snapshot']['treeText'], windows=windows))
    return sorted((p['bounds'] for p in panels), key=lambda p: p['Width'])


def same_panels(actual, expected, tolerance=3):
    assert len(actual) == len(expected) >= 2, (actual, expected)
    for panel, baseline in zip(actual, expected):
        assert abs(panel['X'] - baseline['X']) <= tolerance, (actual, expected)
        assert abs(panel['Y'] - baseline['Y']) <= tolerance, (actual, expected)


def passed(name):
    print('PASS', name, flush=True)


def unchanged(before):
    after = gui.state()
    for key in ('text', 'selected', 'marked', 'undo', 'redo', 'composing', 'focused'):
        assert before[key] == after[key], (key, before, after)


try:
    gui.edit('Native IME', 48); gui.hotkey('CmdOrCtrl+A')
    for key in 'nihongonohenkann': gui.key(key)
    gui.key('Space'); before = gui.state()
    assert before['composing'] and before['marked'][0] >= 0
    canvas_panels = record('canvas-marked'); passed('canvas-marked')
    gui.press('全文をInspectorで編集'); unchanged(before)
    inspector_panels = record('inspector-marked')
    assert inspector_panels[0]['X'] - canvas_panels[0]['X'] > 500, (canvas_panels, inspector_panels)
    passed('inspector-marked')
    gui.key('Tab')
    after = gui.state()
    assert after['focused'] and after['composing'] and after['marked'][0] >= 0, after
    assert after['undo'] == before['undo'] and after['redo'] == before['redo'], after
    same_panels(record('marked-tab-ime-priority'), inspector_panels)
    passed('marked-tab-ime-priority')
    gui.hotkey('Shift+Tab')
    after = gui.state()
    assert after['focused'] and after['composing'] and after['marked'][0] >= 0, after
    assert after['undo'] == before['undo'] and after['redo'] == before['redo'], after
    same_panels(record('marked-shift-tab-ime-priority'), inspector_panels)
    passed('marked-shift-tab-ime-priority')
    before = after
    gui.edit('Native IME', 48); unchanged(before)
    same_panels(record('canvas-return-marked'), canvas_panels)
    passed('canvas-return-marked')
    
    for _ in range(5):
        if not gui.state()['composing']: break
        gui.key('Return')
    gui.key('Escape')
    assert not gui.state()['composing']
    assert before['text'].encode() in gui.save()
    record('composition-saved')
    passed('composition-saved')
    gui.press('全文をInspectorで編集'); gui.hotkey('CmdOrCtrl+A')
    for key in 'nihongonohenkann': gui.key(key)
    gui.key('Space')
    assert gui.state()['composing'], gui.state()
    same_panels(record('second-composition-inspector'), inspector_panels, tolerance=8)
    passed('second-composition-inspector')
    gui.edit('Native IME', 48)
    same_panels(record('second-composition-canvas'), canvas_panels, tolerance=8)
    passed('second-composition-canvas')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
