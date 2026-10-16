import argparse
import json
import os
from pathlib import Path
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
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor.json')
gui = Gui(out, 'presentation { metadata { title "Overview IME" }; slide { heading "Original heading"; }; slide { heading "Second"; }; }')
records = []


def editor():
    return json.loads((out / 'editor.json').read_text())


def observe(name):
    gui.snapshot(name)
    records.append(dict(name=name, editor=editor(), input=gui.state()))
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))


try:
    assert editor()['revision'] == args.revision
    gui.edit('Original heading', 48)
    gui.hotkey('CmdOrCtrl+A')
    subprocess.run(['xcrun', 'swift', '-e', '''
import Carbon
let sources = TISCreateInputSourceList(nil, true).takeRetainedValue() as! [TISInputSource]
let japanese = sources.first { Unmanaged<AnyObject>.fromOpaque(TISGetInputSourceProperty($0, kTISPropertyInputSourceID)).takeUnretainedValue() as! String == "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese" }!
precondition(TISSelectInputSource(japanese) == noErr)
'''], check=True)
    time.sleep(.3)
    for key in 'nihongonohenkann':
        gui.key(key)
    gui.key('Space')
    composed = gui.state()['text']
    assert gui.state()['composing'] and composed.startswith('日本語の'), gui.state()
    observe('japanese-marked-before-switch')
    gui.key('Down')
    assert editor()['currentSlide'] == 0 and gui.state()['composing']
    composed = gui.state()['text']
    gui.menu('表示', 'Overview')
    assert editor()['overview'] and not gui.state()['composing'] and not gui.state()['focused']
    assert composed in bytes.fromhex(editor()['sourceHex']).decode()
    assert gui.fixture.read_bytes() == gui.original
    observe('japanese-committed-overview')
    gui.key('Return')
    assert not editor()['overview'] and editor()['currentSlide'] == 0
    gui.menu('ファイル', '保存')
    assert composed in gui.fixture.read_text()
    observe('japanese-returned-and-saved')
    print('PASS native Japanese composition, arrow priority, Overview commit and save', flush=True)
finally:
    gui.stop()
