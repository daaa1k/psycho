import argparse
import json
import os
import subprocess
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--appearance', choices=['light', 'dark'], required=True)
args = parser.parse_args()
state_path = args.output.resolve() / 'editor.json'
os.environ['PSYCHO_EDITOR_STATE'] = str(state_path)
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = args.appearance
gui = Gui(args.output, 'presentation { metadata { title "Insertion appearance" }; slide { columns { column width=50; column width=50 } }; }\n')
records = []
try:
    resize = gui.out / 'resize-window'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('resize_window.swift')), '-o', str(resize)], check=True)
    for width, height in [(1024, 700), (1280, 892), (1440, 960)]:
        subprocess.run([str(resize), str(gui.app.pid), str(width), str(height)], check=True)
        gui.press('挿入')
        state, rows = gui.snapshot(f'{args.appearance}-{width}x{height}')
        editor = json.loads(state_path.read_text())
        assert editor['dark'] == (args.appearance == 'dark'), editor
        assert not editor['dirty'] and not editor['undo'], editor
        assert state['snapshot']['window']['width'] == width and state['snapshot']['window']['height'] == height, state['snapshot']['window']
        for label in ('見出し', '本文', '箇条書き', 'コード', '画像', '2列'):
            assert any(row['text'].replace(' ', '').endswith(label) and row['x'] < 330 for row in rows), (label, rows)
        records.append({'name': f'{args.appearance}-{width}x{height}', 'window': state['snapshot']['window'], 'editor': editor})
        gui.key('Escape')
        assert gui.save() == gui.original
    gui.press('挿入');gui.press('本文')
    gui.replace(args.appearance + '-body');gui.key('Escape')
    assert ('text "' + args.appearance + '-body"').encode() in gui.save()
    gui.snapshot('appearance-inserted')
    print('PASS insertion icons and labels at three window sizes, view-only history and body insertion in ' + args.appearance, flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
