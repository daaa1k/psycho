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
parser.add_argument('--revision', required=True)
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor-state.json')
os.environ['PSYCHO_NAVIGATOR_TRACE'] = str(out / 'navigator.csv')
source = 'presentation {\n metadata { title "Slide organization" }\n // preserve standalone comment\n' + ''.join(
    f' slide id="s{n}" {{ heading "Heading {n:03}"; text #"raw {n}"#; }} // trailing {n}\n' for n in range(1, 101)) + '}\n'
gui = Gui(out, source)
helper = out / 'pointer'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('pointer.swift')), '-o', str(helper)], check=True)
keys = out / 'keys'
subprocess.run(['xcrun', 'swiftc', str(root / 'validation/navigator/navigator_keys.swift'), '-o', str(keys)], check=True)
records = []


def state():
    return json.loads((out / 'editor-state.json').read_text())


def observe(name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], out / f'{name}.png')
    records.append(dict(name=name, editor=state(), tree=snapshot['snapshot']['treeText']))
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    value = state()
    assert value['revision'] == args.revision
    return value


def invariant(before):
    after = state()
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft', 'currentSlide']:
        assert after[field] == before[field], (field, before, after)


def select_first():
    tree = gui.act('get-app-state')['snapshot']['treeText']
    index = re.search(r'^\s*(\d+) text (?:\(selected\) )?スライド 1(?: |,)', tree, re.M).group(1)
    gui.act('perform-secondary-action', '--element-index', index, '--action', 'pick', '--no-screenshot')


def pointer(kind, x, y):
    subprocess.run([str(helper), str(gui.app.pid), kind, str(x), str(y)], check=True)


def drag(start, end):
    pointer('down', *start)
    pointer('drag', *end)
    time.sleep(.3)


def source_text():
    return bytes.fromhex(state()['sourceHex']).decode()


def context(x, y):
    pointer('right', x, y)
    pointer('right-up', x, y)


def context_action(label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    index = re.search(r'^\s*(\d+) button ' + re.escape(label) + '$', tree, re.M).group(1)
    gui.act('click', '--element-index', index, '--no-screenshot')
    assert state()['navigatorFocused']


try:
    assert state()['revision'] == args.revision, state()['revision']
    select_first()
    pointer('right', 80, 320)
    pointer('right-up', 80, 320)
    context_state = observe('context-target-second')
    assert context_state['currentSlide'] == 1
    tree = records[-1]['tree']
    for label in ['スライド追加', 'スライドを削除', '前へ移動', '後ろへ移動']:
        assert 'button ' + label in tree, tree
    gui.key('Escape')
    assert state()['navigatorFocused']
    assert 'button スライドを削除' not in gui.act('get-app-state')['snapshot']['treeText']
    select_first()
    pointer('right', 80, 175)
    pointer('right-up', 80, 175)
    observe('context-first-disabled-earlier')
    assert 'button (disabled) 前へ移動' in records[-1]['tree']
    gui.key('Escape')
    assert 'button スライドを削除' not in gui.act('get-app-state')['snapshot']['treeText']
    context(80, 175)
    context_action('後ろへ移動')
    assert state()['currentSlide'] == 1
    gui.menu('編集', '元に戻す')
    assert source_text() == source
    context(80, 175)
    context_action('スライド追加')
    assert state()['currentSlide'] == 1
    context(80, 320)
    context_action('前へ移動')
    assert state()['currentSlide'] == 0
    context(80, 175)
    context_action('スライドを削除')
    assert state()['currentSlide'] == 0
    for _ in range(3):
        gui.menu('編集', '元に戻す')
    assert source_text() == source
    select_first()
    gui.menu('ファイル', '保存')
    gui.stop()
    gui.start()
    select_first()
    before = observe('initial')
    drag((80, 165), (80, 175))
    observe('noop-drag')
    pointer('up', 80, 175)
    observe('noop-drop')
    invariant(before)
    drag((80, 165), (80, 330))
    observe('insertion-cue')
    gui.key('Escape')
    pointer('up', 80, 330)
    observe('escape-cancel')
    invariant(before)
    drag((80, 165), (80, 330))
    pointer('drag', 400, 330)
    pointer('up', 400, 330)
    observe('outside-drop')
    invariant(before)
    drag((80, 175), (80, 220))
    pointer('drag', 80, 135)
    pointer('up', 80, 135)
    observe('header-drop')
    invariant(before)
    pointer('move', 80, 820)
    time.sleep(.7)
    observe('header-cancel-pointer-at-end')
    assert (out / 'navigator.csv').read_text().splitlines()[-1].split(',')[0] == '0'
    invariant(before)
    subprocess.run([str(keys), str(gui.app.pid), 'down', '99'], check=True)
    time.sleep(.5)
    tail = observe('tail-selected')
    assert tail['currentSlide'] == 99, tail
    drag((80, 740), (80, 165))
    observe('autoscroll-start')
    time.sleep(17)
    observe('autoscroll-stationary-head')
    pointer('up', 80, 165)
    moved = observe('tail-to-head')
    assert moved['currentSlide'] == 0, moved
    assert source_text().index('id="s100"') < source_text().index('id="s1"')
    assert source_text().count('// trailing ') == 100
    gui.menu('編集', '元に戻す')
    restored = observe('undo-move')
    assert source_text() == source and restored['currentSlide'] == 99
    assert not restored['undo'] and restored['redo'], restored
    gui.menu('編集', 'やり直す')
    observe('redo-move')
    assert state()['sourceHex'] == moved['sourceHex'] and state()['currentSlide'] == 0
    assert state()['undo'] and not state()['redo']
    gui.menu('スライド', 'スライド追加')
    added = observe('menu-add-after-current')
    assert added['currentSlide'] == 1
    assert len(re.findall(r'\bslide(?: |\{)', source_text())) == 101
    gui.hotkey('CmdOrCtrl+Shift+N')
    observe('keyboard-add')
    assert state()['currentSlide'] == 2
    gui.hotkey('CmdOrCtrl+Up')
    observe('keyboard-move-earlier')
    assert state()['currentSlide'] == 1
    gui.key('Backspace')
    assert len(re.findall(r'\bslide(?: |\{)', source_text())) == 101
    gui.hotkey('CmdOrCtrl+Down')
    observe('keyboard-move-later')
    assert state()['currentSlide'] == 2
    gui.key('Backspace')
    observe('keyboard-delete')
    assert len(re.findall(r'\bslide(?: |\{)', source_text())) == 100
    gui.menu('編集', '元に戻す')
    observe('undo-delete')
    assert len(re.findall(r'\bslide(?: |\{)', source_text())) == 101
    saved = gui.save()
    assert saved == bytes.fromhex(state()['sourceHex'])
    observe('saved')
    gui.stop()
    gui.start()
    observe('reloaded')
    assert gui.fixture.read_bytes() == saved
    assert bytes.fromhex(state()['sourceHex']) == saved
    (out / 'result.json').write_text(json.dumps(dict(result='PASS', pid=gui.app.pid, revision=args.revision, macOS=subprocess.check_output(['sw_vers'], text=True), display=subprocess.check_output(['system_profiler', 'SPDisplaysDataType'], text=True)), indent=2))
finally:
    gui.stop()
