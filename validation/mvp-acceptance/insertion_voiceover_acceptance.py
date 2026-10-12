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
        if record['cursor'] == '挿入 group':
            gui.hotkey('Ctrl+Alt+Shift+Down')
        else:
            vo('tell vo cursor to move ' + direction)
    raise AssertionError((label, records[-5:]))

try:
    settings('welcome', False)
    settings('scripting', True)
    source = 'presentation {\n metadata { title "VoiceOver" }\n slide { columns { column width=50; column width=50 } }\n}\n'
    gui = Gui(args.output / 'gui', source)
    subprocess.run([str(toggle), str(gui.app.pid)], check=True)
    time.sleep(1)
    assert json.loads(subprocess.check_output([str(helper)], text=True))['voiceOverEnabled'] is True
    tree = gui.act('get-app-state', '--restore-window')['snapshot']['treeText']
    button = re.search(r'^\s*(\d+) button 挿入$', tree, re.M)
    assert button, tree
    gui.act('click', '--element-index', button[1], '--no-screenshot')
    gui.snapshot('voiceover-menu')
    root_parent = move_to('スライド直下')
    assert 'スライド直下' in root_parent['phrase'] and any(word in root_parent['phrase'] for word in ('選択', 'selected')), root_parent
    left = move_to('左列')
    assert '左列' in left['phrase'], left
    vo('tell vo cursor to perform action')
    tree = gui.act('get-app-state')['snapshot']['treeText']
    assert re.search(r'radio button \(selected, settable\) 2列 1 の左列', tree), tree
    disabled = move_to('2列', button=True)
    assert any(word in disabled['phrase'] for word in ('使用不可', '淡色', '無効', 'dimmed', 'disabled')), disabled
    body = move_to('本文', 'left', button=True)
    assert 'ボタン' in body['phrase'] or 'button' in body['phrase'], body
    vo('tell vo cursor to perform action')
    gui.act('get-app-state')
    gui.replace('voiceover-left')
    gui.key('Escape')
    saved = gui.save()
    assert re.search(rb'column width=50\s*\{\s*text "voiceover-left"\s*\};\s*column width=50', saved), saved
    gui.snapshot('voiceover-inserted-left')
    print('PASS VoiceOver speech, selected parent, disabled kind and left-column insertion', flush=True)
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
