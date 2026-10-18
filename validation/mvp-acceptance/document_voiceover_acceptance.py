import argparse
import json
import re
import subprocess
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
helper = args.output.resolve() / 'vo-settings'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('voiceover_insertion_settings.swift')), '-o', str(helper)], check=True)
toggle = args.output.resolve() / 'vo-toggle'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('insertion_voiceover_toggle.swift')), '-o', str(toggle)], check=True)
existing = json.loads(subprocess.check_output(['orca', 'computer', 'list-apps', '--json'], text=True))
utility_present = any(app.get('bundleId') == 'com.apple.VoiceOverUtility' for app in existing['result']['apps'])
subprocess.run(['open', '-b', 'com.apple.VoiceOverUtility'], check=True)
time.sleep(1)
initial = json.loads(subprocess.check_output([str(helper)], text=True))
assert initial['voiceOverEnabled'] is False, 'Run this scratch recipe with VoiceOver initially off'
records = []
gui = None

def settings(name, value):
    return json.loads(subprocess.check_output([str(helper), name, '1' if value else '0'], text=True))

def vo(script):
    return subprocess.check_output(['osascript', '-e', 'tell application "VoiceOver"\n' + script + '\nend tell'], text=True, timeout=15).strip()

def observe(name):
    time.sleep(.5)
    record = {'name': name, 'cursor': vo('get text under cursor of vo cursor'), 'phrase': vo('get content of last phrase')}
    records.append(record)
    print(json.dumps(record, ensure_ascii=False), flush=True)
    return record

def move_to(label, direction='right', button=False):
    for _ in range(45):
        record = observe('navigation')
        matched = record['cursor'].startswith(label + ' ') and 'radio button' not in record['cursor'] if button else label in record['cursor']
        if matched:
            return record
        if record['cursor'] in ('挿入 group', '診断 group'):
            gui.hotkey('Ctrl+Alt+Shift+Down')
        else:
            vo('tell vo cursor to move ' + direction)
    raise AssertionError((label, records[-5:]))

try:
    settings('welcome', False)
    settings('scripting', True)
    source = 'presentation { metadata { title "Document VoiceOver" }; slide { heading "Original"; image "missing.png"; }; }\n'
    gui = Gui(args.output / 'gui', source)
    gui.edit('Original', 48)
    gui.replace('VoiceOver retained draft')
    subprocess.run([str(toggle), str(gui.app.pid)], check=True)
    time.sleep(1)
    assert json.loads(subprocess.check_output([str(helper)], text=True))['voiceOverEnabled'] is True
    gui.hotkey('CmdOrCtrl+W')
    gui.snapshot('voiceover-native-modal')
    modal = observe('native-modal')
    assert any(label in modal['cursor'] + modal['phrase'] for label in ('破棄', '保存', '未保存')), modal
    cancel = move_to('キャンセル', button=True)
    assert any(word in cancel['phrase'] for word in ('ボタン', 'button')), cancel
    vo('tell vo cursor to perform action')
    gui.act('get-app-state')
    assert gui.state()['text'] == 'VoiceOver retained draft'
    assert gui.state()['focused'], gui.state()
    assert gui.fixture.read_bytes() == gui.original
    gui.menu('表示', '診断一覧')
    gui.snapshot('voiceover-diagnostics')
    issue = move_to('画像', button=True)
    assert 'missing.png' in issue['cursor'] + issue['phrase'], issue
    assert any(word in issue['phrase'] for word in ('ボタン', 'button')), issue
    vo('tell vo cursor to perform action')
    assert gui.state()['text'] == 'missing.png', gui.state()
    assert gui.fixture.read_bytes() == gui.original
    gui.snapshot('voiceover-diagnostic-target')
    print('PASS VoiceOver native modal cancel/focus and image diagnostic navigation', flush=True)

finally:
    try:
        current = json.loads(subprocess.check_output([str(helper)], text=True))
        if current['voiceOverEnabled']:
            if gui:
                gui.act('get-app-state', '--restore-window')
                subprocess.run([str(toggle), str(gui.app.pid)], check=True)
                time.sleep(1)
            else:
                subprocess.run(['osascript', '-e', 'tell application "VoiceOver" to quit'], check=True)
        settings('scripting', initial['scripting'])
        restored = settings('welcome', initial['welcome'])
        assert restored['voiceOverEnabled'] is False, restored
        assert restored['scripting'] == initial['scripting'] and restored['welcome'] == initial['welcome'], restored
        (args.output / 'restoration.json').write_text(json.dumps({'initial': initial, 'restored': restored}, ensure_ascii=False, indent=2) + '\n')
        if not utility_present:
            subprocess.run(['osascript', '-e', 'tell application "VoiceOver Utility" to quit'], check=True)
    finally:
        if gui:
            gui.stop()
        (args.output / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
