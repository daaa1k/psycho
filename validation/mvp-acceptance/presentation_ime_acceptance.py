import argparse
import json
import subprocess
import time
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/issue29-presentation-ime')
args = parser.parse_args()
gui = Gui(args.output,
          'presentation { metadata { title "IME presentation" }; slide { heading "Original heading"; }; }\n')
records = []
try:
    gui.click(900, 750)
    gui.edit('Original heading', 48)
    gui.hotkey('CmdOrCtrl+A')
    subprocess.run(['xcrun', 'swift', '-e', '''
import Carbon
let sources = TISCreateInputSourceList(nil, true).takeRetainedValue() as! [TISInputSource]
let japanese = sources.first {
    Unmanaged<AnyObject>.fromOpaque(TISGetInputSourceProperty($0, kTISPropertyInputSourceID))
        .takeUnretainedValue() as! String == "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese"
}!
precondition(TISSelectInputSource(japanese) == noErr)
'''], check=True)
    time.sleep(.3)
    for key in 'nihongonohenkann':
        gui.key(key)
    gui.key('Space')
    assert gui.state()['composing'] and gui.state()['marked'][0] >= 0, gui.state()
    composed = gui.state()['text']
    assert composed.startswith('日本語の') and len(composed) >= 5, gui.state()
    gui.snapshot('marked-before-start')
    records.append(dict(case='marked-before-start', input=gui.state()))
    gui.press('最初から発表')
    time.sleep(1)
    state, rows = gui.snapshot('composition-committed-in-presentation')
    assert state['snapshot']['window']['width'] == 1920, state
    assert any(''.join(row['text'].split()) == composed for row in rows), rows
    assert not gui.state()['composing'] and not gui.state()['focused'], gui.state()
    assert gui.fixture.read_bytes() == gui.original
    records.append(dict(case='presentation', input=gui.state(), unsaved_bytes_preserved=True))
    gui.key('Escape')
    time.sleep(1)
    assert not gui.state()['focused'], gui.state()
    gui.key('Right')
    state, rows = gui.snapshot('exit-keeps-canvas-focus')
    assert state['snapshot']['window']['width'] == 1280
    assert any(''.join(row['text'].split()) == composed and 160 < row['x'] < 995 for row in rows)
    assert gui.fixture.read_bytes() == gui.original
    gui.press('保存')
    assert gui.fixture.read_bytes() == gui.original.replace(b'Original heading', composed.encode())
    records.append(dict(case='returned-and-saved', input=gui.state(), saved=gui.fixture.read_text()))
    print('PASS native IME commit, unsaved presentation, canvas return and explicit save', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
