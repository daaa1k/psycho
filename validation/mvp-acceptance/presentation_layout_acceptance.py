import argparse
import csv
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/issue29-presentation-layout')
parser.add_argument('--revision', default=subprocess.check_output(
    ['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip())
args = parser.parse_args()
directory = args.output.resolve()
state_path = directory / 'editor-state.json'
os.environ['PSYCHO_EDITOR_STATE'] = str(state_path)
source = Path(__file__).with_name('layout.kdl').read_text().replace(
    '../../assets/build-time.png', os.path.relpath(root / 'assets/build-time.png', directory))
gui = Gui(directory, source)
observations = []


def observe(mode, slide, trace_start):
    name = f'layout-{mode}-{slide}'
    snapshot, _ = gui.snapshot(name)
    editor = json.loads(state_path.read_text())
    assert editor['revision'] == args.revision, editor
    presenting = mode == 'fullscreen'
    assert editor['presenting'] == presenting, editor
    rows = list(csv.reader(gui.trace.open()))
    physical = snapshot['screenshot'].get('scale', 1)
    window = snapshot['snapshot']['window']
    captions = [row for row in rows[trace_start:] if len(row) == 12
                and bytes.fromhex(row[3]).decode().startswith('画像の Caption')
                and abs(float(row[0]) - window['width']) < .01
                and abs(float(row[11]) - physical) < .000001
                and abs(float(row[8]) - (min(float(row[0]) / 1280, float(row[1]) / 720)
                    if presenting else editor['scale'])) < .000001]
    assert captions, (name, editor, window)
    caption = captions[-1]
    viewport = dict(width=float(caption[0]), height=float(caption[1]))
    canvas = viewport if presenting else dict(width=editor['canvasWidth'], height=editor['canvasHeight'])
    scale = min(canvas['width'] / 1280, canvas['height'] / 720)
    assert abs(float(caption[8]) - scale) < .000001, (name, caption, editor)
    observations.append(dict(mode=mode, slide=slide, screenshot=f'{name}.png',
                             viewport=viewport, canvas=canvas, scale=scale,
                             physicalScale=physical, editor=editor, window=window,
                             traceStart=trace_start, traceEnd=len(rows)))
    (gui.out / 'layout-observations.json').write_text(json.dumps(observations, ensure_ascii=False, indent=2) + '\n')


try:
    gui.click(900, 750)
    for mode, width, height in [('compact', 1024, 700), ('small', 1280, 892), ('large', 1440, int(os.environ.get('PSYCHO_ACCEPTANCE_LARGE_HEIGHT','992')))]:
        subprocess.run(['xcrun', 'swift', str(Path(__file__).with_name('move_window.swift')),
                        str(gui.app.pid), '10', '33'], check=True, capture_output=True)
        subprocess.run(['xcrun', 'swift', str(Path(__file__).with_name('resize_window.swift')),
                        str(gui.app.pid), str(width), str(height)], check=True, capture_output=True)
        time.sleep(.5)
        for slide in [1, 2]:
            trace_start = len(list(csv.reader(gui.trace.open())))
            gui.select_slide(slide)
            observe(mode, slide, trace_start)
            window = observations[-1]['window']
            assert (window['width'], window['height']) == (width, height), window
    trace_start = len(list(csv.reader(gui.trace.open())))
    gui.press('最初から発表')
    time.sleep(1)
    observe('fullscreen', 1, trace_start)
    trace_start = len(list(csv.reader(gui.trace.open())))
    gui.key('Right')
    observe('fullscreen', 2, trace_start)
    gui.key('Escape')
    time.sleep(1)
    shutil.copyfile(gui.trace, gui.out / 'glyph-origins.csv')
    assert gui.fixture.read_bytes() == gui.original
    subprocess.run([sys.executable, str(Path(__file__).with_name('measure_layout.py')),
                    '--evidence', str(gui.out)], check=True)
    print('PASS identical Japanese glyphs, wrapping and base placement in three editor sizes and fullscreen', flush=True)
finally:
    gui.stop()
