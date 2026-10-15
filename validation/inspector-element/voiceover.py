import argparse
import json
import re
import subprocess
import time
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--settings-only', action='store_true')
parser.add_argument('--width-only', action='store_true')
parser.add_argument('--offscreen-only', action='store_true')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
helper = args.output.resolve() / 'vo-settings'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).resolve().parents[1] / 'mvp-acceptance/voiceover_insertion_settings.swift'), '-o', str(helper)], check=True)
toggle = args.output.resolve() / 'vo-toggle'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).resolve().parents[1] / 'mvp-acceptance/insertion_voiceover_toggle.swift'), '-o', str(toggle)], check=True)
stop_voiceover = args.output.resolve() / 'stop-voiceover'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('stop_voiceover.swift')), '-o', str(stop_voiceover)], check=True)
existing = json.loads(subprocess.check_output(['orca', 'computer', 'list-apps', '--json'], text=True))
utility_present = any(app.get('bundleId') == 'com.apple.VoiceOverUtility' for app in existing['result']['apps'])
enabled = subprocess.check_output(['xcrun', 'swift', '-e', 'import AppKit; print(NSWorkspace.shared.isVoiceOverEnabled ? \"true\" : \"false\")'], text=True).strip()
assert enabled == 'false', 'Run this scratch recipe with VoiceOver initially off'
initial = None
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
    previous = None
    for _ in range(45):
        record = observe('navigation')
        matched = record['cursor'].startswith(label + ' ') and 'radio button' not in record['cursor'] if button else label in record['cursor']
        if matched:
            return record
        if record['cursor'] == previous:
            gui.hotkey('Ctrl+Alt+Shift+Up')
            vo('tell vo cursor to move ' + direction)
        elif record['cursor'].endswith(' group') or record['cursor'].endswith(' list'):
            gui.hotkey('Ctrl+Alt+Shift+Down')
        else:
            vo('tell vo cursor to move ' + direction)
        previous = record['cursor']
    raise AssertionError((label, records[-5:]))

try:
    subprocess.run(['open', '-b', 'com.apple.VoiceOverUtility'], check=True)
    time.sleep(1)
    initial = json.loads(subprocess.check_output([str(helper)], text=True))
    settings('welcome', False)
    settings('scripting', True)
    source = 'presentation { metadata { title "Inspector VoiceOver" }; slide { columns { column width=50 { text "Same"; }; column width=50 { text "Same"; }; }; columns { column width=50 { text "Same"; }; column width=50 { text "Same"; }; }; code "print();" language="unknown"; }; }\n'
    if args.width_only:
        source = 'presentation { metadata { title \"VoiceOver width\" }; slide { columns { column width=50; column width=50; }; }; }\n'
    if args.offscreen_only:
        source = 'presentation { metadata { title "Offscreen VoiceOver" }; slide { ' + ''.join('text "Row ' + str(i) + '"; ' for i in range(1, 21)) + 'columns { column width=50 { text "Left"; }; column width=50 { text "Right"; }; }; }; }\n'
    gui = Gui(args.output / 'gui', source)
    if not args.offscreen_only:
        gui.outline_select('Element 1 · 2列' if args.width_only else 'Element 3 · コード' if args.settings_only else '2列 1 · 左列 · 本文 1')
    for _ in range(2):
        gui.act('get-app-state', '--restore-window')
        subprocess.run([str(toggle), str(gui.app.pid)], check=True)
        for _ in range(10):
            if json.loads(subprocess.check_output([str(helper)], text=True))['voiceOverEnabled']:
                break
            time.sleep(.5)
        else:
            continue
        break
    assert json.loads(subprocess.check_output([str(helper)], text=True))['voiceOverEnabled'] is True
    gui.act('get-app-state', '--restore-window')
    if args.offscreen_only:
        move_to('2列 21 · 左列 · 本文 1')
        vo('tell vo cursor to perform action')
        assert '(selected) 2列 21 · 左列 · 本文 1' in gui.act('get-app-state')['snapshot']['treeText']
        gui.snapshot('voiceover-offscreen-nested-selection')
        move_to('2列 21 · 右列 · 本文 1')
        vo('tell vo cursor to perform action')
        assert '(selected) 2列 21 · 右列 · 本文 1' in gui.act('get-app-state')['snapshot']['treeText']
        move_to('本文 text field')
        gui.hotkey('Ctrl+Alt+Shift+Down')
        assert gui.state()['focused'], gui.state()
        gui.replace('VoiceOver offscreen edited')
        assert b'text "VoiceOver offscreen edited"' in gui.save()
        gui.snapshot('voiceover-offscreen-edit-saved')
        print('PASS native VoiceOver offscreen nested selection, edit and Save', flush=True)
    elif args.width_only:
        move_to('Element 1 · 2列')
        vo('tell vo cursor to perform action')
        assert '(selected) Element 1 · 2列' in gui.act('get-app-state')['snapshot']['treeText']
        move_to('左列幅を数値入力', button=True)
        vo('tell vo cursor to perform action')
        width = move_to('左列幅、1〜99の整数', 'left')
        assert '1' in width['phrase'] and '99' in width['phrase'], width
        gui.hotkey('Ctrl+Alt+Shift+Down')
        assert gui.state()['focused'], gui.state()
        gui.replace('60')
        saved = gui.save()
        assert b'column width=60' in saved and b'column width=40' in saved
        gui.snapshot('voiceover-width-constraint-and-edit')
        print('PASS native VoiceOver width constraint, numeric edit and saved complementary widths', flush=True)
    elif not args.settings_only:
        row = move_to('2列 1 · 左列 · 本文 1')
        assert '左列' in row['phrase'], row
        vo('tell vo cursor to perform action')
        assert '(selected) 2列 1 · 左列 · 本文 1' in gui.act('get-app-state')['snapshot']['treeText']
        gui.snapshot('voiceover-select-first-left')
        row = move_to('2列 2 · 左列 · 本文 1')
        assert '2' in row['phrase'] and '左列' in row['phrase'], row
        vo('tell vo cursor to perform action')
        assert '(selected) 2列 2 · 左列 · 本文 1' in gui.act('get-app-state')['snapshot']['treeText']
        gui.snapshot('voiceover-select-second-left')
        row = move_to('Element 3 · コード')
        vo('tell vo cursor to perform action')
        control = move_to('言語をrustに変更', button=True)
        assert any(word in control['phrase'] for word in ('ボタン', 'button')), control
        vo('tell vo cursor to perform action')
        assert b'language="rust"' in gui.save()
        gui.snapshot('voiceover-change-language')
        print('PASS native VoiceOver speech, distinct Columns parents, selection and language operation', flush=True)
    else:
        move_to('コード言語', button=True)
        vo('tell vo cursor to perform action')
        language = move_to('コード言語、任意の文字列', 'left')
        assert 'コード言語、任意の文字列' in language['phrase'], language
        gui.hotkey('Ctrl+Alt+Shift+Down')
        assert gui.state()['focused'], gui.state()
        gui.replace('voiceover-custom')
        saved = gui.save()
        assert b'language="voiceover-custom"' in saved
        gui.snapshot('voiceover-language-constraint-and-edit')
        gui.hotkey('Ctrl+Alt+Shift+Up')
        move_to('コード全文', button=True)
        vo('tell vo cursor to perform action')
        full = move_to('コード全文 text field', 'left')
        assert 'コード全文' in full['phrase'], full
        gui.hotkey('Ctrl+Alt+Shift+Down')
        assert gui.state()['focused'], gui.state()
        gui.replace('voiceover_code();\nsecond_line();')
        assert b'voiceover_code();\\nsecond_line();' in gui.save()
        gui.snapshot('voiceover-full-code-edited-and-saved')
        gui.hotkey('Ctrl+Alt+Shift+Up')
        print('PASS native VoiceOver arbitrary language constraint, full code edit and saved values', flush=True)
finally:
    cleanup_errors = []

    def restore(operation):
        try:
            return operation()
        except Exception as error:
            cleanup_errors.append(repr(error))

    if initial is not None:
        restore(lambda: subprocess.run([str(stop_voiceover)], check=True))
        time.sleep(1)
        restore(lambda: settings('scripting', initial['scripting']))
        restore(lambda: settings('welcome', initial['welcome']))

        def verify_restoration():
            restored = json.loads(subprocess.check_output([str(helper)], text=True))
            (args.output / 'restoration.json').write_text(json.dumps({'initial': initial, 'restored': restored}, ensure_ascii=False, indent=2) + '\n')
            assert restored['voiceOverEnabled'] is False, restored
            assert restored['scripting'] == initial['scripting'] and restored['welcome'] == initial['welcome'], restored

        restore(verify_restoration)
    if not utility_present:
        restore(lambda: subprocess.run(['osascript', '-e', 'tell application "VoiceOver Utility" to quit'], check=True))
    if gui:
        restore(gui.stop)
    restore(lambda: (args.output / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n'))
    if cleanup_errors:
        raise RuntimeError('Scratch cleanup failed: ' + '; '.join(cleanup_errors))
