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
parser.add_argument('--appearance', choices=['light', 'dark'], required=True)
parser.add_argument('--revision', required=True)
args = parser.parse_args()
os.environ['PSYCHO_EDITOR_STATE'] = str(args.output.resolve() / 'editor-state.json')
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = args.appearance
heading = 'これは全文ツールチップで確認する長い日本語の見出しです'
source = 'presentation { metadata { title "Navigator layout"; };\n' + '\n'.join(
    f'slide {{ heading "{heading if index == 2 else f"Heading {index:03}"}"; }}'
    for index in range(1, 101)) + '\n}\n'
gui = Gui(args.output, source)
records = []

def state():
    return json.loads((gui.out / 'editor-state.json').read_text())

def observe(name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], gui.out / f'{name}.png')
    record = dict(name=name, editor=state(), tree=snapshot['snapshot']['treeText'],
                  screenshotScale=snapshot['screenshot'].get('scale', 1), window=snapshot['snapshot']['window'])
    records.append(record)
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record

try:
    assert state()['revision'] == args.revision
    assert state()['dark'] == (args.appearance == 'dark')
    before = state()
    resize = gui.out / 'resize'
    hover = gui.out / 'hover'
    keys = gui.out / 'keys'
    for source_file, binary in [(root / 'validation/mvp-acceptance/resize_window.swift', resize),
                                (Path(__file__).with_name('navigator_hover.swift'), hover),
                                (Path(__file__).with_name('navigator_keys.swift'), keys)]:
        subprocess.run(['xcrun', 'swiftc', str(source_file), '-o', str(binary)], check=True)
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(hover), str(gui.app.pid), '80', '320'], check=True)
    tooltip = observe('full-heading-tooltip')
    snapshot, words = gui.snapshot()
    recognized = ''.join(word['text'] for word in words)
    assert heading in recognized, recognized
    assert tooltip['tree'].count(heading) >= 1
    for width, height in [(1024, 700), (1280, 860), (1440, 960)]:
        subprocess.run([str(resize), str(gui.app.pid), str(width), str(height + 32)], check=True)
        time.sleep(.4)
        initial = observe(f'{width}x{height}')
        match = re.search(r'^\s*(\d+) text (?:\(selected\) )?スライド 1 Heading 001,', initial['tree'], re.MULTILINE)
        assert match, initial['tree']
        gui.act('perform-secondary-action', '--element-index', match.group(1), '--action', 'pick', '--no-screenshot')
        gui.act('get-app-state', '--restore-window')
        subprocess.run([str(keys), str(gui.app.pid), 'down', '99'], check=True)
        end = observe(f'{width}x{height}-last')
        assert 'スライド 100 Heading 100' in end['tree'] and state()['currentSlide'] == 99
        subprocess.run([str(keys), str(gui.app.pid), 'up', '99'], check=True)
    for label in ['Navigatorを隠す', 'Navigatorを表示', 'Navigatorを広くする', 'Navigatorを狭くする']:
        gui.menu('表示', label)
        observe(label)
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft']:
        assert state()[field] == before[field], field
    assert gui.fixture.read_bytes() == gui.original
    (gui.out / 'run.json').write_text(json.dumps(dict(revision=args.revision, appearance=args.appearance, result='PASS'), indent=2))
    print('PASS native tooltip, three window sizes, pane visibility/width and selection exposure', flush=True)
finally:
    gui.stop()
