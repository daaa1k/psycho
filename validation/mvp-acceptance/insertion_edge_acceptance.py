import argparse
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--ime', action='store_true')
parser.add_argument('--conflict-only', action='store_true')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
state_path = args.output.resolve() / 'editor.json'
os.environ['PSYCHO_EDITOR_STATE'] = str(state_path)
source = 'presentation {\n metadata { title "Edges" }\n slide id="a" {\n' + ' columns { column width=50; column width=50 }\n' * 4 + ' }\n}\n'
if args.ime:
    source = 'presentation {\n metadata { title "IME" }\n slide {}\n}\n'
gui = Gui(args.output, source)
records = []
main_window = gui.act('get-app-state')['snapshot']['window']['id']

def ax(label):
    selector = ('--window-id', str(main_window)) if args.ime else ()
    state = gui.act('get-app-state', *selector)
    tree = state['snapshot']['treeText']
    if args.ime and label == '挿入' and gui.state()['composing']:
        rows = json.loads(subprocess.check_output([str(gui.ocr), state['screenshot']['path']], text=True))
        scale = state['screenshot'].get('scale', 1)
        buttons = [row for row in rows if row['text'].endswith('挿入') and row['y'] / scale < 110]
        assert len(buttons) == 1, rows
        button = buttons[0]
        gui.act('click', '--x', str(button['x'] / scale), '--y', str(button['y'] / scale), '--no-screenshot', *selector)
        return
    found = re.search(r'^\s*(\d+) (?:button|radio button) (?:\([^)]*\) )?' + re.escape(label) + r'(?: ✓)?(?:, (?:Secondary Actions:|Value:).*)?$', tree, re.M)
    assert found, (label, tree)
    gui.act('click', '--element-index', found[1], '--no-screenshot', *selector)

def native(*path):
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(native_helper), str(gui.app.pid), *path], check=True)
    gui.act('get-app-state')

def document():
    return bytes.fromhex(json.loads(state_path.read_text())['sourceHex'])

def checkpoint(name):
    if args.ime:
        state = gui.act('get-app-state', '--window-id', str(main_window))
        shutil.copy(state['screenshot']['path'], gui.out / (name + '.png'))
    else:
        state, _ = gui.snapshot(name)
    records.append({'name': name, 'tree': state['snapshot']['treeText'], 'source': document().decode(), 'input': gui.state(), 'window': state['snapshot']['window']})
    print('PASS', name, flush=True)

native_helper = gui.out / 'native-menu-path'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_menu_path.swift')), '-o', str(native_helper)], check=True)
try:
    if args.ime:
        ax('挿入');ax('本文')
        gui.key('k');gui.key('a')
        assert gui.state()['marked'] != [-1, -1], gui.state()
        checkpoint('ime-marked')
        ax('挿入')
        assert 'か'.encode() in document(), document()
        ax('見出し');gui.key('Escape')
        assert 'text "か"'.encode() in gui.save(), gui.fixture.read_bytes()
        checkpoint('ime-insert-preserves-composition')
    else:
        if not args.conflict_only:
            resize = gui.out / 'resize-window'
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('resize_window.swift')), '-o', str(resize)], check=True)
            subprocess.run([str(resize), str(gui.app.pid), '1024', '700'], check=True)
            ax('挿入')
            checkpoint('small-window-four-columns')
            tree = gui.act('get-app-state')['snapshot']['treeText']
            for number in (1, 2, 3, 4):
                for side in ('左列', '右列'):
                    assert f'2列 {number} の{side}' in tree
            for _ in range(10):
                gui.key('Tab')
            checkpoint('keyboard-text-scrolled-visible')
            _, rows = gui.snapshot()
            body = next(row for row in rows if row['text'] == '本文')
            assert body['y'] < 670, body
            gui.key('Return')
            assert gui.state()['focused'], gui.state()
            gui.replace('keyboard-root')
            gui.key('Escape')
            saved = gui.save()
            assert b'text "keyboard-root"' in saved
            assert saved.count(b'text ') == 1, saved
            checkpoint('keyboard-insert-root')
            for number in (1, 2, 3, 4):
                for side in ('左列', '右列'):
                    native('挿入', f'2列 {number} の{side}', '本文  説明を文章で伝える')
                    gui.replace(f'native{number}{side}')
                    gui.key('Escape');gui.save()
            saved = gui.fixture.read_bytes()
            nested = re.findall(rb'columns\s*\{\s*column width=50\s*\{\s*text "([^"]*)"\s*\};\s*column width=50\s*\{\s*text "([^"]*)"\s*\}\s*\}', saved)
            expected = [(f'native{number}左列'.encode(), f'native{number}右列'.encode()) for number in (1, 2, 3, 4)]
            assert nested == expected, saved
            checkpoint('native-all-four-columns')
            ax('Element 1 · 2列')
            ax('左列に追加…')
            tree = gui.act('get-app-state')['snapshot']['treeText']
            assert re.search(r'radio button \([^)]*selected[^)]*\) 2列 1 の左列', tree), tree
            ax('本文');gui.replace('inspector-left');gui.key('Escape');gui.save()
            inspected = gui.fixture.read_bytes()
            pairs = re.findall(rb'columns\s*\{\s*column width=50\s*\{(.*?)\};\s*column width=50\s*\{(.*?)\}\s*\}', inspected, re.S)
            values = [(re.findall(rb'text "([^"]*)"', left), re.findall(rb'text "([^"]*)"', right)) for left, right in pairs]
            assert values == [([f'native{number}左列'.encode()] + ([b'inspector-left'] if number == 1 else []), [f'native{number}右列'.encode()]) for number in (1, 2, 3, 4)], inspected
            checkpoint('inspector-parent-selection')
            ax('Element 1 · 2列')
            ax('左列幅を数値入力');gui.replace('0')
            baseline = document()
            for entry in ('toolbar', 'inspector', 'native'):
                if entry == 'toolbar':ax('挿入')
                elif entry == 'inspector':ax('左列に追加…')
                else:native('挿入', 'スライド直下', '本文  説明を文章で伝える')
                assert document() == baseline, (entry, document())
                assert gui.state()['focused'] and gui.state()['text'] == '0', (entry, gui.state())
                checkpoint('invalid-width-' + entry)
            ax('入力を取り消す')
        baseline = document()
        ax('Element 1 · 2列')
        ax('左列幅を数値入力');gui.replace('55')
        changed_on_disk = gui.fixture.read_text().replace('title "Edges"', 'title "External"')
        gui.fixture.write_text(changed_on_disk)
        time.sleep(1.5)
        state = gui.act('get-app-state')
        assert re.search(r'button \(disabled\) 挿入', state['snapshot']['treeText']), state['snapshot']['treeText']
        tree = state['snapshot']['treeText']
        assert '左列に追加…' not in tree or re.search(r'button \(disabled\) 左列に追加…', tree), tree
        assert document() == baseline
        menu_state = gui.out / 'native-menu'
        if not menu_state.exists():
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('edit_menu_state.swift')), '-o', str(menu_state)], check=True)
        metadata = json.loads(subprocess.check_output([str(gui.out / 'native-menu'), str(gui.app.pid)], text=True))
        def nodes(node):
            yield node
            for child in node.get('children', []):
                yield from nodes(child)
        insert_menu = next(node for node in nodes(metadata['menuBar']) if node.get('title') == '挿入')
        native_options = [node for node in nodes(insert_menu) if node.get('title', '').startswith('本文  ')]
        assert len(native_options) == 9 and all(node.get('enabled') is False for node in native_options), native_options
        (gui.out / 'conflict-native.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
        checkpoint('conflict-shared-disabled')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
