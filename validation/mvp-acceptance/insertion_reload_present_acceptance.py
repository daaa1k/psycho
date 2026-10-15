import argparse
import json
import os
import shutil
import subprocess
from pathlib import Path
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
state_path = args.output.resolve() / 'editor.json'
os.environ['PSYCHO_EDITOR_STATE'] = str(state_path)
source = args.source.read_text()
for kind in ('heading', 'text', 'bullets', 'code', 'image', 'columns'):
    assert kind in source, (kind, source)
gui = Gui(args.output, source)
records = []

def checkpoint(name, slide, presenting):
    state, rows = gui.snapshot(name)
    text = ''.join(''.join(row['text'].split()) for row in rows)
    editor = json.loads(state_path.read_text())
    assert editor['presenting'] is presenting, editor
    if presenting:
        expected = 'Title' if slide == 0 else '1左列'
        assert expected in text, (expected, text)
    else:
        assert editor['currentSlide'] == slide, editor
    records.append({'name': name, 'tree': state['snapshot']['treeText'], 'editor': editor, 'text': text})
    print('PASS', name, flush=True)

try:
    asset = gui.out / 'assets/image.png'
    asset.parent.mkdir()
    shutil.copy(root / 'assets/build-time.png', asset)
    gui.outline_select('Element 5 · 画像')
    gui.press('画像を再読み込み', scroll=True)
    assert gui.save() == source.encode()
    gui.stop();gui.start()
    assert gui.fixture.read_bytes() == source.encode()
    checkpoint('all-kinds-reloaded', 0, False)
    gui.menu('プレゼンテーション', '最初からプレゼンテーション')
    checkpoint('all-content-kinds-presented', 0, True)
    gui.key('Right')
    checkpoint('both-columns-presented', 1, True)
    gui.key('Escape')
    checkpoint('presentation-return', 1, False)
    assert gui.fixture.read_bytes() == source.encode()
    native = gui.out / 'native-menu-path'
    windows_helper = gui.out / 'native-menu-windows'
    for filename, executable in [('native_menu_path.swift', native), ('insertion_native_windows.swift', windows_helper)]:
        subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name(filename)), '-o', str(executable)], check=True)
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(native), str(gui.app.pid), '挿入', 'スライド直下'], check=True)
    windows = json.loads(subprocess.check_output([str(windows_helper), str(gui.app.pid)], text=True))
    assert windows, 'No owned native menu windows'
    (gui.out / 'native-menu-windows.json').write_text(json.dumps(windows, indent=2) + '\n')
    for index, window in enumerate(windows):
        subprocess.run(['screencapture', '-x', '-l', str(window['kCGWindowNumber']), str(gui.out / f'native-menu-{index}.png')], check=True)
    gui.key('Escape');gui.key('Escape')
    assert gui.fixture.read_bytes() == source.encode()
    print('PASS native insertion menu images captured', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
