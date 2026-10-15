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
parser.add_argument('--outline-only', action='store_true')
parser.add_argument('--footer-only', action='store_true')
args = parser.parse_args()
long_code = 'overflow();\n' * 30
source = ('presentation { metadata { title "Controls" }; slide { code ' + json.dumps(long_code) + '; '
          'columns { column width=50 { text "Same"; }; column width=50 { text "Same"; }; }; '
          'columns { column width=50 { text "Same"; }; column width=50 { text "Same"; }; }; ' +
          ''.join('text "Hidden ' + str(i) + '"; ' for i in range(4, 24)) + '}; }\n')
os.environ['PSYCHO_EDITOR_STATE'] = str(args.output.resolve() / 'editor.json')
gui = Gui(args.output, source)
records = []
helper = gui.out / 'native-identity'


def tree():
    return gui.act('get-app-state')['snapshot']['treeText']


def ax(label):
    for _ in range(18):
        value = tree()
        match = re.search(r'^\s*(\d+) (?:button|text field) (?:\([^)]*\) )?' + re.escape(label) + r'(?:, .*)?$', value, re.M)
        if match:
            gui.act('click', '--element-index', match[1], '--no-screenshot')
            if 'text field' in match.group(0) and not gui.state()['focused']:
                nodes = json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))
                node = next(n for n in nodes if n['role'] == 'AXTextField' and n['label'] == label)
                window = next(n for n in nodes if n['role'] == 'AXWindow')
                gui.click(node['position'][0] - window['position'][0] + 8,
                          node['position'][1] - window['position'][1] -
                          (window['size'][1] - gui.act('get-app-state')['screenshot']['height']) + min(8, node['size'][1] / 2))
            return
        gui.inspector_scroll(580, 'down')
    raise AssertionError((label, value))


def record(name):
    state, _ = gui.snapshot(name)
    native = json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))
    records.append(dict(name=name, tree=state['snapshot']['treeText'], input=gui.state(), editor=json.loads((gui.out / 'editor.json').read_text()), native=native, kdl=gui.fixture.read_text()))
    print('PASS', name, flush=True)


try:
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_identity.swift')), '-o', str(helper)], check=True)
    if not args.footer_only:
        if not args.outline_only:
            gui.outline_select('Element 2 · 2列')
            ax('左列幅を数値入力'); ax('左列幅、1〜99の整数'); gui.replace('0'); gui.key('Tab')
            assert not gui.state()['focused'], gui.state()
            gui.hotkey('CmdOrCtrl+Alt+1')
            assert 'list スライド一覧' in tree()
            gui.hotkey('CmdOrCtrl+1')
            assert not json.loads((gui.out / 'editor.json').read_text())['actual']
            gui.hotkey('CmdOrCtrl+Alt+2')
            assert 'スライドの構造一覧' in tree() and gui.state()['text'] == '0'
            assert gui.fixture.read_bytes() == gui.original
            record('invalid-width-tab-hide-refused')
            ax('入力を取り消す'); ax('左列幅を数値入力'); ax('左列幅、1〜99の整数'); gui.replace('60'); gui.key('Tab')
            gui.hotkey('CmdOrCtrl+Alt+2'); assert 'スライドの構造一覧' not in tree()
            gui.hotkey('CmdOrCtrl+Alt+2'); assert 'スライドの構造一覧' in tree()
            assert b'column width=60' in gui.save()
            record('valid-width-tab-hide-accepted')
            ax('Presentation title'); ax('Presentation のタイトル'); gui.replace(''); gui.key('Tab')
            gui.hotkey('CmdOrCtrl+Alt+2')
            assert 'スライドの構造一覧' in tree() and gui.state()['text'] == ''
            record('empty-title-tab-hide-refused')
            gui.hotkey('CmdOrCtrl+Alt+1'); assert 'list スライド一覧' in tree()
            gui.hotkey('CmdOrCtrl+1'); assert not json.loads((gui.out / 'editor.json').read_text())['actual']
            ax('入力を取り消す')
            unchanged = gui.save()
            gui.hotkey('CmdOrCtrl+Alt+1'); assert 'list スライド一覧' not in tree()
            gui.hotkey('CmdOrCtrl+Alt+1')
            gui.hotkey('CmdOrCtrl+1'); assert json.loads((gui.out / 'editor.json').read_text())['actual']
            gui.hotkey('CmdOrCtrl+0'); assert not json.loads((gui.out / 'editor.json').read_text())['actual']
            assert gui.save() == unchanged
            record('valid-view-only-changes')
            gui.outline_select('Element 1 · コード'); ax('コード全文')
            assert gui.state()['text'] == long_code and gui.state()['focused'], gui.state()
            gui.key('Tab'); record('long-code-tab-next-setting')
            gui.hotkey('Shift+Tab'); assert gui.state()['focused']
            assert gui.state()['text'] == long_code
            record('long-code-shift-tab-full-input')
            gui.key('Tab'); gui.key('Tab'); record('long-code-tab-language-setting')
        gui.outline_select('2列 2 · 左列 · 本文 1')
        record('two-columns-first-left')
        gui.outline_select('2列 3 · 左列 · 本文 1')
        record('two-columns-second-left')
        identities = {n['label']: n.get('AXIdentifier') for n in records[-1]['native'] if '本文 1' in n['label']}
        assert identities['2列 2 · 左列 · 本文 1'] != identities['2列 3 · 左列 · 本文 1'], identities
        gui.key('Down'); assert '(selected) 2列 3 · 右列 · 本文 1' in tree()
        record('outline-arrow-right-child')
    resize = gui.out / 'resize-window'
    subprocess.run(['xcrun', 'swiftc', str(root / 'validation/mvp-acceptance/resize_window.swift'), '-o', str(resize)], check=True)
    subprocess.run([str(resize), str(gui.app.pid), '1024', '700'], check=True)
    for _ in range(10):
        gui.inspector_scroll(655, 'down')
    native = json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))
    window = next(n for n in native if n['role'] == 'AXWindow')
    diagnostic = next(n for n in native if n['label'].startswith('Layout · Slide 1 · Element 23 ·'))
    assert diagnostic['position'][1] >= window['position'][1]
    assert diagnostic['position'][1] + diagnostic['size'][1] <= window['position'][1] + window['size'][1]
    record('small-window-footer-last-diagnostic')
    ax(diagnostic['label'])
    assert gui.state()['text'] == 'Hidden 23'
    record('small-window-last-element-settings')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
