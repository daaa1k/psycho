import argparse
import json
import subprocess
import time
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
original_text = '日本語の変換'
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Reconversion" }; slide { text "日本語の変換"; }; }\n')
observations = []

def record(name, candidates=False):
    state, rows = gui.snapshot(name)
    windows = json.loads(subprocess.check_output([
        'xcrun', 'swift', str(Path(__file__).with_name('ime_windows.swift')),
        str(gui.app.pid)], text=True))
    entry = dict(case=name, input=gui.state(), windows=windows)
    observations.append(entry)
    if candidates:
        panels = [window for window in windows if window['layer'] == 20]
        assert panels, 'Native reconversion candidates must be visible'
        panel = min(panels, key=lambda window: window['bounds']['X'])
        subprocess.run(['screencapture', '-l', str(panel['id']), '-x',
                        str(gui.out / (name + '-candidate.png'))], check=True, capture_output=True)
        main = next(window for window in windows if window['layer'] == 0 and window['bounds']['Width'] >= 1000)
        subprocess.run(['xcrun', 'swift', '-e', 'import Cocoa; precondition(NSRunningApplication(processIdentifier:pid_t(CommandLine.arguments[1])!)?.isActive == true)',
                        str(gui.app.pid)], check=True, capture_output=True)
        region = ','.join(str(round(main['bounds'][key])) for key in ['X', 'Y', 'Width', 'Height'])
        subprocess.run(['screencapture', '-R', region, '-x', str(gui.out / (name + '-visible.png'))],
                       check=True, capture_output=True)
    return entry, rows

try:
    gui.edit(original_text, 28)
    gui.hotkey('CmdOrCtrl+A')
    assert gui.state()['selected'] == [0, 6]
    assert gui.state()['undo'] == 0
    gui.hotkey('Ctrl+Shift+R')
    time.sleep(.6)
    reconverting, _ = record('reconverting', True)
    assert reconverting['input']['composing'] and reconverting['input']['marked'][0] >= 0
    assert reconverting['input']['focused'] and reconverting['input']['undo'] == 0
    for attempt in range(10):
        gui.key('Down')
        chosen = gui.state()['text']
        if chosen != original_text:
            break
    else:
        raise AssertionError('A different native reconversion candidate was not available')
    candidate, rows = record('different-candidate', True)
    assert candidate['input']['composing'] and candidate['input']['undo'] == 0
    assert any(''.join(row['text'].split()) == ''.join(chosen.split()) and 160 < row['x'] < 995 for row in rows)
    for attempt in range(3):
        gui.key('Return')
        if gui.state()['marked'] == [-1, -1]:
            break
    else:
        raise AssertionError('Native reconversion did not finalize')
    confirmed, _ = record('confirmed')
    assert confirmed['input']['text'] == chosen and not confirmed['input']['composing']
    assert confirmed['input']['undo'] == 1
    gui.hotkey('CmdOrCtrl+Z')
    assert gui.state()['text'] == original_text and gui.state()['undo'] == 0
    gui.hotkey('CmdOrCtrl+Shift+Z')
    assert gui.state()['text'] == chosen and gui.state()['undo'] == 1
    assert gui.fixture.read_bytes() == gui.original, 'Reconversion must not save automatically'
    gui.key('Escape')
    expected = gui.original.replace(json.dumps(original_text, ensure_ascii=False).encode(),
                                    json.dumps(chosen, ensure_ascii=False).encode())
    assert gui.save() == expected
    gui.press('Undo')
    assert gui.save() == gui.original, 'One document Undo must restore reconverted text'
    gui.press('Redo')
    assert gui.save() == expected
    gui.stop()
    gui.start()
    assert gui.fixture.read_bytes() == expected
    gui.edit(chosen, 28)
    assert gui.state()['text'] == chosen and gui.state()['undo'] == 0
    record('reloaded')
    print('PASS native reconversion, candidate change, one Undo/Redo, Save and Reload', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(observations, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
