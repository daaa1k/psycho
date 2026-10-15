import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
code = 'overflow();\n' * 30
gui = Gui(args.output, 'presentation { metadata { title "Long code focus" }; slide { code ' + json.dumps(code) + '; }; }\n')
records = []
helper = gui.out / 'native-identity'


def capture(name, label, role):
    state, _ = gui.snapshot(name)
    native = json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))
    window = next(n for n in native if n['role'] == 'AXWindow')
    focused = next(n for n in native if n.get('AXFocused') and n['role'] != 'AXWindow')
    records.append(dict(name=name, input=gui.state(), native=native, focused=focused))
    assert focused['label'] == label and focused['role'] == role, focused
    x, y = focused['position']; width, height = focused['size']
    wx, wy = window['position']; ww, wh = window['size']
    if role == 'AXButton':
        assert wx <= x and x + width <= wx + ww and wy <= y and y + height <= wy + wh, (focused, window)
    else:
        assert y < wy + wh and y + height > wy, (focused, window)
        assert gui.state()['text'] == code and gui.state()['focused']
    print('PASS', name, flush=True)


try:
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_identity.swift')), '-o', str(helper)], check=True)
    gui.outline_select('Element 1 · コード')
    native = json.loads(subprocess.check_output([str(helper), str(gui.app.pid)], text=True))
    field = next(n for n in native if n['role'] == 'AXTextField')
    window = next(n for n in native if n['role'] == 'AXWindow')
    screenshot = gui.act('get-app-state')['screenshot']
    titlebar = window['size'][1] - screenshot['height']
    gui.click(field['position'][0] - window['position'][0] + 8, field['position'][1] - window['position'][1] - titlebar + 8)
    assert gui.state()['focused'] and gui.state()['text'] == code
    gui.key('Tab'); capture('long-code-tab-visible-apply', '適用', 'AXButton')
    gui.hotkey('Shift+Tab'); capture('long-code-shift-tab-visible-input', 'コード全文', 'AXTextField')
    for label in ['適用', 'コード全文', '言語をrustに変更', '言語をjsonに変更', '言語をyamlに変更', '言語をtomlに変更', '言語をなしに変更', 'コード言語']:
        gui.key('Tab'); capture('long-code-tab-' + label, label, 'AXButton')
    assert gui.save() == gui.original
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
