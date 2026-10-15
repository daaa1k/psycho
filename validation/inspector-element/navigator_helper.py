import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
os.environ['PSYCHO_EDITOR_STATE'] = str(args.output.resolve() / 'editor.json')
source = 'presentation { metadata { title "Navigator helper" }; ' + ''.join('slide { heading "Slide ' + str(i) + '"; }; ' for i in range(1, 9)) + '}\n'
gui = Gui(args.output, source)
records = []
try:
    helper = gui.out / 'resize-window'
    subprocess.run(['xcrun', 'swiftc', str(root / 'validation/mvp-acceptance/resize_window.swift'), '-o', str(helper)], check=True)
    subprocess.run([str(helper), str(gui.app.pid), '1024', '700'], check=True)
    for index, slide in enumerate([1, 5, 1]):
        gui.select_slide(slide)
        state, _ = gui.snapshot('selection-' + str(index) + '-slide-' + str(slide))
        editor = json.loads((gui.out / 'editor.json').read_text())
        assert editor['currentSlide'] == slide - 1, editor
        assert gui.fixture.read_text() == source
        records.append(dict(slide=slide, editor=editor, tree=state['snapshot']['treeText']))
    print('PASS shared Navigator helper selects 1, offscreen 5, then offscreen 1 at 1024x700', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
