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
parser.add_argument('--burst', action='store_true')
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


def state():
    return json.loads((out / 'editor.json').read_text())


try:
    enabled = subprocess.check_output(['xcrun', 'swift', '-e', 'import AppKit; print(NSWorkspace.shared.isVoiceOverEnabled ? "true" : "false")'], text=True).strip()
    assert enabled == 'false', 'This scratch recipe requires VoiceOver initially off'
    subprocess.run(['open', '-b', 'com.apple.VoiceOverUtility'], check=True)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        response = subprocess.run(['orca', 'computer', 'get-app-state', '--app', 'com.apple.VoiceOverUtility', '--json'], capture_output=True, text=True)
        snapshot = json.loads(response.stdout)
        tree = snapshot.get('result', {}).get('snapshot', {}).get('treeText', '')
        if 'AppleScriptによるVoiceOverの制御を許可' in tree and 'VoiceOverの起動時にようこそダイアログを表示' in tree:
            break
        time.sleep(.25)
    else:
        raise RuntimeError('VoiceOver Utility General settings did not become accessible')
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
    initial_focus = observe('initial-overview-focus')
    assert 'スライド 1 Heading 001' in initial_focus['cursor'], initial_focus
    vo('tell vo cursor to move right')
    row = observe('navigate-to-second')
    assert '2' in row['phrase'] and 'Heading' in row['phrase'], row
    vo('tell vo cursor to perform action')
    assert state()['currentSlide'] == 1 and state()['overviewFocused']
    activated = gui.act('get-app-state')
    (out / 'activated-native.json').write_text(json.dumps(activated, ensure_ascii=False, indent=2))
    _, focus_marker, focused = activated['snapshot']['treeText'].partition('The focused UI element is ')
    assert focus_marker and 'スライド 2 Heading 002' in focused, activated
    gui.hotkey('Ctrl+Alt+Shift+Up')
    selected = observe('activated-selected-second')
    assert any(word in selected['phrase'].lower() for word in ['選択', 'selected']), selected
    gui.snapshot('voiceover-selected')
    gui.hotkey('Ctrl+Alt+Shift+Down')
    gui.hotkey('CmdOrCtrl+Down')
    assert state()['currentSlide'] == 2 and state()['overviewFocused']
    moved = bytes.fromhex(state()['sourceHex']).decode()
    assert re.findall(r'id="([^"]+)"', moved)[:3] == ['s1', 's3', 's2']
    for key, prefix, selected_index in [
        ('CmdOrCtrl+Up', ['s1', 's2', 's3'], 1),
        ('CmdOrCtrl+Down', ['s1', 's3', 's2'], 2),
        ('CmdOrCtrl+Z', ['s1', 's2', 's3'], 1),
        ('CmdOrCtrl+Shift+Z', ['s1', 's3', 's2'], 2),
    ]:
        gui.hotkey(key)
        current = state()
        assert re.findall(r'id="([^"]+)"', bytes.fromhex(current['sourceHex']).decode())[:3] == prefix
        assert current['currentSlide'] == selected_index and current['overviewFocused']
        observe('voiceover-' + key)
    observe('keyboard-reorder-with-voiceover')
    key_helper = out / 'keys'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('keys.swift')), '-o', str(key_helper)], check=True)
    transitions = []
    if args.burst:
        subprocess.run([str(key_helper), str(gui.app.pid), 'right', '100'], check=True)
        time.sleep(2)
        current = state()
        (out / 'after-burst.json').write_text(json.dumps(current, indent=2))
        assert current['currentSlide'] == 99, current
    else:
        for expected in range(3, 100):
            subprocess.run([str(key_helper), str(gui.app.pid), 'right', '1'], check=True)
            deadline = time.monotonic() + 2
            while state()['currentSlide'] != expected and time.monotonic() < deadline:
                time.sleep(.05)
            actual = state()['currentSlide']
            transitions.append(dict(expected=expected, actual=actual))
            (out / 'keyboard-transitions.json').write_text(json.dumps(transitions, indent=2))
            assert actual == expected, transitions[-1]
    native_tail, _ = gui.snapshot('voiceover-tail')
    (out / 'tail-native.json').write_text(json.dumps(native_tail, ensure_ascii=False, indent=2))
    _, focus_marker, focused = native_tail['snapshot']['treeText'].partition('The focused UI element is ')
    assert focus_marker and 'スライド 100 Heading 100' in focused, native_tail
    tail_focus = observe('virtualized-tail-keyboard-focus')
    assert 'スライド 100 Heading 100' in tail_focus['cursor'], tail_focus
    gui.hotkey('Ctrl+Alt+Shift+Up')
    tail = observe('virtualized-tail-selection')
    assert '100' in tail['phrase'] and 'Heading' in tail['phrase'] and '選択' in tail['phrase'], tail
    gui.hotkey('Ctrl+Alt+Shift+Down')
    before_add = bytes.fromhex(state()['sourceHex'])
    gui.hotkey('CmdOrCtrl+Shift+N')
    added = state()
    added_source = bytes.fromhex(added['sourceHex'])
    assert added['currentSlide'] == 100 and added['overviewFocused'], added
    assert len(re.findall(rb'\bslide\b', added_source)) == 101, added_source
    added_speech = observe('keyboard-add-slide-with-voiceover')
    assert '101' in added_speech['phrase'], added_speech
    gui.hotkey('CmdOrCtrl+S')
    deadline = time.monotonic() + 2
    while gui.fixture.read_bytes() != added_source and time.monotonic() < deadline:
        time.sleep(.05)
    assert gui.fixture.read_bytes() == added_source
    gui.hotkey('CmdOrCtrl+Z')
    assert bytes.fromhex(state()['sourceHex']) == before_add
    assert state()['currentSlide'] == 99 and state()['overviewFocused']
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
