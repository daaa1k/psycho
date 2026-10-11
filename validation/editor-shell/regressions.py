"""Exercise repaired assets, dark diagnostics and the six-slide native-open workflow."""
import argparse
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
args = parser.parse_args()
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = 'dark'


def launch(name, source):
    out = args.output.resolve() / name
    os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor-state.json')
    return Gui(out, source, name=name)


def editor(gui):
    return json.loads((gui.out / 'editor-state.json').read_text())


def observe(gui, name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], gui.out / f'{name}.png')
    (gui.out / f'{name}.json').write_text(json.dumps(dict(editor=editor(gui),
        input=gui.state(), snapshot=snapshot['snapshot']), ensure_ascii=False, indent=2))
    return snapshot['snapshot']['treeText']


def click(gui, label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) button ' + re.escape(label) + r'$', tree, re.MULTILINE)
    assert match, tree
    gui.act('click', '--element-index', match[1], '--no-screenshot')


gui = launch('asset', 'presentation { metadata { title "Asset repair" }; slide { image "missing.png"; }; }\n')
try:
    shutil.copy(root / 'assets/build-time.png', gui.out / 'repaired.png')
    assert 'button (disabled) 発表' in observe(gui, 'missing-disabled')
    _, rows = gui.snapshot()
    diagnostic = next(row for row in rows if 'missing.png' in row['text'] and 'Slide' in row['text'])
    gui.click(diagnostic['x'], diagnostic['y'])
    gui.press('画像パス')
    assert gui.state()['focused']
    gui.replace('repaired.png')
    assert editor(gui)['draft']
    observe(gui, 'repaired-draft')
    click(gui, '発表')
    assert editor(gui)['presenting']
    assert gui.fixture.read_bytes() == gui.original
    observe(gui, 'repaired-presenting-without-save')
    gui.key('Escape')
    time.sleep(1)
    assert not editor(gui)['presenting']
    assert b'image "repaired.png"' in gui.save()
    print('PASS missing asset draft directly starts presentation without saving', flush=True)
finally:
    gui.stop()

for name, source, label in [
    ('syntax', 'presentation {\n metadata { title "x" }\n slide { text "bad\\q"; }\n}\n', 'KDL'),
    ('schema', 'presentation {\n metadata { title "x" }\n slide { text 42; }\n}\n', 'Schema')]:
    gui = launch(name, source)
    try:
        tree = observe(gui, 'invalid-disabled')
        assert 'button (disabled) 発表' in tree
        _, rows = gui.snapshot()
        diagnostic = next(row for row in rows if label in row['text'] and gui.fixture.name in row['text'])
        gui.click(diagnostic['x'], diagnostic['y'])
        _, rows = gui.snapshot('selected-source')
        assert any('読み取り専用' in row['text'] for row in rows), rows
        observe(gui, 'selected-source-evidence')
        gui.hotkey('CmdOrCtrl+Alt+P')
        assert not editor(gui)['presenting']
        assert gui.fixture.read_bytes() == gui.original
        print('PASS dark ' + name + ' diagnostic selection and disabled presentation', flush=True)
    finally:
        gui.stop()

gui = launch('representative', 'presentation { metadata { title "Before native open" }; slide { text "Initial"; }; }\n')
try:
    sample = gui.out / 'six-slides.kdl'
    sample.write_text((root / 'examples/build-time.kdl').read_text().replace('../assets/build-time.png', 'assets/build-time.png'))
    (gui.out / 'assets').mkdir()
    shutil.copy(root / 'assets/build-time.png', gui.out / 'assets/build-time.png')
    gui.menu('ファイル', '開く')
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.act('type-text', '--text', str(sample), '--no-screenshot')
    gui.key('Return')
    gui.key('Return')
    time.sleep(1)
    observe(gui, 'native-open-six-slides')
    assert bytes.fromhex(editor(gui)['sourceHex']) == sample.read_bytes()
    drag = gui.out / 'slow-drag'
    subprocess.run(['xcrun', 'swiftc', str(root / 'validation/mvp-acceptance/slow_drag.swift'), '-o', str(drag)], check=True)
    before = editor(gui)
    subprocess.run([str(drag), str(gui.app.pid), str(before['navigatorWidth'] + 12), '400',
                    str(before['navigatorWidth'] + 52), '400'], check=True)
    time.sleep(.4)
    assert editor(gui)['navigatorWidth'] > before['navigatorWidth']
    observe(gui, 'navigator-width-drag')
    before = editor(gui)
    subprocess.run([str(drag), str(gui.app.pid), str(1280 - before['inspectorWidth'] - 12), '400',
                    str(1280 - before['inspectorWidth'] - 52), '400'], check=True)
    time.sleep(.4)
    assert editor(gui)['inspectorWidth'] > before['inspectorWidth']
    observe(gui, 'inspector-width-drag')
    gui.edit('変更のたびに', 28)
    gui.replace('レビューの待ち時間を短縮する。')
    gui.hotkey('CmdOrCtrl+S')
    saved = sample.read_bytes()
    assert 'レビューの待ち時間を短縮する。' in saved.decode()
    gui.key('Escape')
    gui.hotkey('CmdOrCtrl+Z')
    gui.hotkey('CmdOrCtrl+S')
    assert sample.read_text() == (root / 'examples/build-time.kdl').read_text().replace('../assets/build-time.png', 'assets/build-time.png')
    gui.hotkey('CmdOrCtrl+Shift+Z')
    gui.hotkey('CmdOrCtrl+S')
    assert sample.read_bytes() == saved
    observe(gui, 'edited-saved-undo-redo')
    gui.menu('発表', '最初から発表')
    assert editor(gui)['presenting']
    for number in range(6):
        if number:
            gui.key('Right')
        observe(gui, f'presentation-slide-{number + 1}')
    gui.key('Escape')
    time.sleep(1)
    assert not editor(gui)['presenting']
    assert sample.read_bytes() == saved
    observe(gui, 'presentation-return')
    print('PASS native open six slides, pane drags, body edit, keyboard save/history and presentation return', flush=True)
finally:
    gui.stop()
