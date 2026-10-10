import argparse
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import time
import unicodedata

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui, root

SOURCE = '''presentation {
 metadata { title "Retreat acceptance" }
 slide id="intro" { heading "Original" }
 // Image references outside the edited slide must follow the same Assets.
 slide id="image" { image #"assets/first.jpg"# { caption "First" } }
 slide id="columns" {
  columns {
   column width=50 { image "assets/second.jpg" }
   column width=50 { image "assets/first.jpg" }
  }
 }
}
'''


def normalized(text):
    return unicodedata.normalize('NFKC', ''.join(text.split()))


def run(gui):
    records = []
    archive = gui.out / 'archive'
    archive.mkdir()
    destination = archive / 'presentation.kdl'
    original_assets = {path: path.read_bytes() for path in (gui.out / 'assets').iterdir()}

    def observe(name, *expected):
        state, rows = gui.snapshot(name)
        assert state['screenshot']['scale'] == 1, state['screenshot']
        text = normalized(' '.join(row['text'] for row in rows))
        record = dict(name=name, text=text, input=gui.state(), tree=state['snapshot']['treeText'])
        records.append(record)
        (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
        for fragment in expected:
            assert normalized(fragment) in text, (name, fragment, text)
        print('PASS', name, flush=True)
        return state

    def button(name):
        state = gui.act('get-app-state')
        line = next(line for line in state['snapshot']['treeText'].splitlines()
                    if line.strip().endswith('button ' + name))
        gui.act('click', '--element-index', line.strip().split()[0], '--no-screenshot')

    def choose(folder):
        gui.press('別名保存')
        button('退避先を選ぶ')
        gui.hotkey('CmdOrCtrl+Shift+G')
        state = gui.act('get-app-state')
        assert 'text' in state['snapshot']['treeText'].lower()
        gui.hotkey('CmdOrCtrl+A')
        gui.act('type-text', '--text', str(folder), '--no-screenshot')
        gui.key('Return')
        state = gui.act('get-app-state')
        assert 'Where:, Value: ' + folder.name in state['snapshot']['treeText'], state

    def cancel_replace():
        button('Cancel')
        state = gui.act('get-app-state')
        if 'button Save' in state['snapshot']['treeText']:
            button('Cancel')

    def retained(before):
        after = gui.state()
        for field in ['text', 'undo', 'redo']:
            assert after[field] == before[field], (field, before, after)

    gui.edit('Original', 48)
    gui.replace('Retained draft')
    gui.replace('Future draft')
    gui.hotkey('CmdOrCtrl+Z')
    before = gui.state()
    assert before['text'] == 'Retained draft' and before['undo'] > 0 and before['redo'] > 0
    gui.press('別名保存')
    observe('01-before-save-notice', '履歴', '消去', '編集していない画像参照')
    button('キャンセル')
    retained(before)
    assert gui.fixture.read_bytes() == gui.original
    gui.press('別名保存')
    button('退避先を選ぶ')
    button('Cancel')
    retained(before)
    assert gui.fixture.read_bytes() == gui.original
    observe('02-panel-cancel-preserves-input')

    mode = stat.S_IMODE(archive.stat().st_mode)
    try:
        choose(archive)
        os.chmod(archive, 0o500)
        button('Save')
        observe('03-write-failure', '退避保存できません')
        retained(before)
        assert gui.fixture.read_bytes() == gui.original and not destination.exists()
    finally:
        os.chmod(archive, mode)

    external = gui.original.replace(b'heading "Original"', b'heading "External original"')
    gui.fixture.write_bytes(external)
    time.sleep(1)
    observe('04-conflict', '競合')
    retained(before)
    choose(gui.out)
    button('Save')
    observe('05-native-overwrite-confirmation', 'Replace')
    cancel_replace()
    retained(before)
    assert gui.fixture.read_bytes() == external
    choose(gui.out)
    button('Save')
    button('Replace')
    observe('06-original-rejected', '退避保存できません')
    retained(before)
    assert gui.fixture.read_bytes() == external

    sentinel = b'existing destination must survive cancellation\n'
    destination.write_bytes(sentinel)
    choose(archive)
    button('Save')
    observe('07-existing-destination-confirmation', 'Replace')
    cancel_replace()
    retained(before)
    assert destination.read_bytes() == sentinel and gui.fixture.read_bytes() == external

    choose(archive)
    button('Save')
    button('Replace')
    observe('08-retreat-success', '別ファイルへ保存しました', 'Retained draft')
    saved = destination.read_bytes()
    expected = gui.original.decode().replace('heading "Original"', 'heading "Retained draft"')
    expected = expected.replace('#"assets/first.jpg"#', '"../assets/first.jpg"')
    expected = expected.replace('"assets/first.jpg"', '"../assets/first.jpg"')
    expected = expected.replace('"assets/second.jpg"', '"../assets/second.jpg"')
    assert saved.decode() == expected, saved.decode()
    assert gui.fixture.read_bytes() == external
    paths = re.findall(r'image "([^"]+)"', saved.decode())
    assert len(paths) == 3
    for path in paths:
        asset = (archive / path).resolve()
        assert asset.read_bytes() == original_assets[asset]
    assert set(archive.iterdir()) == {destination}
    assert gui.state()['undo'] == 0 and gui.state()['redo'] == 0
    gui.hotkey('CmdOrCtrl+Z')
    gui.hotkey('CmdOrCtrl+Shift+Z')
    gui.hotkey('CmdOrCtrl+S')
    assert destination.read_bytes() == saved and gui.fixture.read_bytes() == external
    gui.select_slide(2)
    observe('09-rebased-image', 'First')
    gui.select_slide(3)
    observe('10-nested-images')
    gui.select_slide(1)
    gui.edit('Retained draft', 48)
    gui.replace('Resumed editing')
    gui.hotkey('CmdOrCtrl+S')
    assert b'heading "Resumed editing"' in destination.read_bytes()
    assert gui.fixture.read_bytes() == external
    observe('11-editing-resumed', 'Resumed editing')
    for path, data in original_assets.items():
        assert path.read_bytes() == data


def run_focused_image(gui):
    def button(name):
        state = gui.act('get-app-state')
        line = next(line for line in state['snapshot']['treeText'].splitlines()
                    if line.strip().endswith('button ' + name))
        gui.act('click', '--element-index', line.strip().split()[0], '--no-screenshot')

    gui.select_slide(2)
    gui.click(580, 350)
    gui.press('assets/first.jpg')
    assert gui.state()['focused'], gui.state()
    archive = gui.out / 'archive'
    archive.mkdir()
    gui.press('別名保存')
    button('退避先を選ぶ')
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(archive), '--no-screenshot')
    gui.key('Return')
    button('Save')
    gui.snapshot('focused-image-after-save')
    assert gui.state()['text'] == '../assets/first.jpg', gui.state()
    destination = archive / 'presentation.kdl'
    saved = destination.read_bytes()
    assert b'"../assets/first.jpg"' in saved
    gui.hotkey('CmdOrCtrl+S')
    assert destination.read_bytes() == saved
    assert gui.fixture.read_bytes() == gui.original
    result = 'PASS focused image path remains rebased after Save As and next Save'
    (gui.out / 'result.txt').write_text(result + '\n')
    print(result, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--case', choices=['recovery', 'focused-image'], default='recovery')
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to(root / 'target')
    output.mkdir(parents=True, exist_ok=False)
    assets = output / 'assets'
    assets.mkdir()
    for name in ['first.jpg', 'second.jpg']:
        shutil.copy(root / 'tests/fixtures/orientation-6.jpg', assets / name)
    gui = Gui(output, SOURCE)
    try:
        if args.case == 'focused-image':
            run_focused_image(gui)
        else:
            run(gui)
    except Exception:
        gui.snapshot('failure')
        raise
    finally:
        gui.stop()
