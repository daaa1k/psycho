import argparse
import json
import os
from pathlib import Path
import re
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
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor.json')
helpers = {}
for name, filename in [('settings', 'voiceover_insertion_settings.swift'), ('toggle', 'insertion_voiceover_toggle.swift')]:
    helpers[name] = out / name
    subprocess.run(['xcrun', 'swiftc', str(root / 'validation/mvp-acceptance' / filename), '-o', str(helpers[name])], check=True)
helpers['stop'] = out / 'stop'
subprocess.run(['xcrun', 'swiftc', str(root / 'validation/inspector-element/stop_voiceover.swift'), '-o', str(helpers['stop'])], check=True)
listed = json.loads(subprocess.check_output(['orca', 'computer', 'list-apps', '--json'], text=True))
utility_present = any(app.get('bundleId') == 'com.apple.VoiceOverUtility' for app in listed['result']['apps'])
initial = None
gui = None
records = []


def settings(name, value):
    return json.loads(subprocess.check_output([str(helpers['settings']), name, '1' if value else '0'], text=True))


def vo(script):
    return subprocess.check_output(['osascript', '-e', 'tell application "VoiceOver"\n' + script + '\nend tell'], text=True, timeout=15).strip()


def observe(name):
    time.sleep(.4)
    record = dict(name=name, cursor=vo('get text under cursor of vo cursor'), phrase=vo('get content of last phrase'))
    records.append(record)
    (out / 'speech.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    print(json.dumps(record, ensure_ascii=False), flush=True)
    return record


def move_to(label, direction="right"):
    previous = None
    for _ in range(60):
        record = observe('navigate')
        if label in record['cursor']:
            return record
        if record['cursor'] == previous:
            gui.hotkey('Ctrl+Alt+Shift+Up')
            vo('tell vo cursor to move ' + direction)
        elif label == 'Overviewのスライド一覧' and record['cursor'] == 'スライド一覧 list':
            vo('tell vo cursor to move right')
        elif record['cursor'].endswith(' group') or record['cursor'].endswith(' list'):
            gui.hotkey('Ctrl+Alt+Shift+Down')
        else:
            vo('tell vo cursor to move ' + direction)
        previous = record['cursor']
    raise AssertionError((label, records[-5:]))


def state():
    return json.loads((out / 'editor.json').read_text())


try:
    enabled = subprocess.check_output(['xcrun', 'swift', '-e', 'import AppKit; print(NSWorkspace.shared.isVoiceOverEnabled ? "true" : "false")'], text=True).strip()
    assert enabled == 'false', 'This scratch recipe requires VoiceOver initially off'
    subprocess.run(['open', '-b', 'com.apple.VoiceOverUtility'], check=True)
    time.sleep(1)
    initial = json.loads(subprocess.check_output([str(helpers['settings'])], text=True))
    settings('welcome', False)
    settings('scripting', True)
    source = 'presentation { metadata { title "Overview VoiceOver" }; ' + ''.join(f'slide id="s{n}" {{ heading "Heading {n:03}"; }};' for n in range(1, 101)) + ' }'
    gui = Gui(out / 'gui', source)
    gui.menu('表示', 'Overview')
    assert state()['revision'] == args.revision
    for _ in range(2):
        gui.act('get-app-state', '--restore-window')
        subprocess.run([str(helpers['toggle']), str(gui.app.pid)], check=True)
        for _ in range(10):
            if json.loads(subprocess.check_output([str(helpers['settings'])], text=True))['voiceOverEnabled']:
                break
            time.sleep(.5)
        if json.loads(subprocess.check_output([str(helpers['settings'])], text=True))['voiceOverEnabled']:
            break
    assert json.loads(subprocess.check_output([str(helpers['settings'])], text=True))['voiceOverEnabled']
    gui.act('get-app-state', '--restore-window')
    move_to('Overviewのスライド一覧')
    gui.hotkey('Ctrl+Alt+Shift+Down')
    row = move_to('スライド 2 Heading 002')
    assert '2' in row['phrase'] and 'Heading' in row['phrase'], row
    vo('tell vo cursor to perform action')
    assert state()['currentSlide'] == 1 and state()['overviewFocused']
    gui.hotkey('Ctrl+Alt+Shift+Up')
    selected = observe('activated-selected-second')
    assert any(word in selected['phrase'].lower() for word in ['選択', 'selected']), selected
    gui.snapshot('voiceover-selected')
    gui.hotkey('Ctrl+Alt+Shift+Down')
    gui.hotkey('CmdOrCtrl+Down')
    assert state()['currentSlide'] == 2 and state()['overviewFocused']
    moved = bytes.fromhex(state()['sourceHex']).decode()
    assert re.findall(r'id="([^"]+)"', moved)[:3] == ['s1', 's3', 's2']
    observe('keyboard-reorder-with-voiceover')
    key_helper = out / 'keys'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('keys.swift')), '-o', str(key_helper)], check=True)
    subprocess.run([str(key_helper), str(gui.app.pid), 'right', '100'], check=True)
    gui.act('get-app-state')
    assert state()['currentSlide'] == 99
    gui.hotkey('Ctrl+Alt+Shift+Up')
    tail = observe('virtualized-tail-selection')
    assert '100' in tail['phrase'] and 'Heading' in tail['phrase'] and '選択' in tail['phrase'], tail
    gui.hotkey('Ctrl+Alt+Shift+Down')
    gui.key('Return')
    assert not state()['overview'] and state()['currentSlide'] == 99 and state()['canvasFocused']
    gui.snapshot('voiceover-return-to-edit')
    print('PASS native VoiceOver speech, activation, selected state, reorder and Return', flush=True)
finally:
    errors = []
    def restore(operation):
        try:
            return operation()
        except Exception as error:
            errors.append(repr(error))
    if initial is not None:
        restore(lambda: subprocess.run([str(helpers['stop'])], check=True))
        time.sleep(1)
        restore(lambda: settings('scripting', initial['scripting']))
        restore(lambda: settings('welcome', initial['welcome']))
        def check_restoration():
            restored = json.loads(subprocess.check_output([str(helpers['settings'])], text=True))
            (out / 'restoration.json').write_text(json.dumps(dict(initial=initial, restored=restored), indent=2))
            assert not restored['voiceOverEnabled']
            assert restored['scripting'] == initial['scripting'] and restored['welcome'] == initial['welcome']
        restore(check_restoration)
    if not utility_present:
        restore(lambda: subprocess.run(['osascript', '-e', 'tell application "VoiceOver Utility" to quit'], check=True))
    if gui:
        restore(gui.stop)
    if errors:
        raise RuntimeError('; '.join(errors))
