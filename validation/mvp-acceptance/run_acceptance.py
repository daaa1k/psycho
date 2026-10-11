import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
scripts = Path(__file__).resolve().parent
cases = {
    'sample': ('sample_workflow_acceptance.py', []),
    'blank': ('blank_slide_acceptance.py', []),
    'code-colors': ('code_colors_acceptance.py', []),
    'layout': ('presentation_layout_acceptance.py', []),
    'direct-layout': ('direct_layout_acceptance.py', []),
    'ime': ('ime_acceptance.py', []),
    'ime-caption': ('ime_acceptance.py', ['--caption-roundtrip']),
    'presentation-ime': ('presentation_ime_acceptance.py', []),
    'keys': ('presentation_workflow_acceptance.py', ['--section', 'keys']),
    'presentation': ('presentation_workflow_acceptance.py', ['--section', 'snapshot', '--section', 'preflight', '--section', 'positions', '--section', 'external']),
    'text': ('text_workflow_acceptance.py', []),
    'no-edit-save': ('no_edit_save_acceptance.py', []),
    'redo': ('input_redo_transfer_acceptance.py', ['--end', 'escape']),
    'columns': ('columns_workflow_acceptance.py', []),
    'layout-repair': ('layout_repair_acceptance.py', []),
    'following-layout': ('following_layout_acceptance.py', []),
    'save-as-retry': ('save_as_retry_acceptance.py', []),
    'cursor': ('close_cursor_acceptance.py', ['--cursor-only']),
    'close': ('close_cursor_acceptance.py', []),
    'input-scope': ('input_scope_acceptance.py', []),
    'history-limit': ('input_history_limit_acceptance.py', []),
    'bullet-items': ('bullet_items_acceptance.py', []),
    'nested-bullet-items': ('bullet_items_acceptance.py', ['--nested']),
    'assets': ('assets_acceptance.py', []),
    'save-failure': ('save_failure_acceptance.py', []),
    'conflict': ('conflict_acceptance.py', []),
}
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--case', action='append', choices=cases)
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to(root / 'target'), 'Evidence must be under target/'
out.mkdir(parents=True, exist_ok=False)
binary = root / 'target/debug/psycho'
assert binary.is_file(), 'Build the product with --features gui first'
screens = json.loads(subprocess.check_output(['xcrun', 'swift', '-e', '''
import AppKit
import CoreGraphics
let rows = NSScreen.screens.map { screen -> [String: Any] in
    let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as! CGDirectDisplayID
    let bounds = CGDisplayBounds(id)
    return ["name": screen.localizedName, "x": bounds.minX, "y": bounds.minY,
            "width": bounds.width, "height": bounds.height, "scale": screen.backingScaleFactor]
}
print(String(data: try! JSONSerialization.data(withJSONObject: rows), encoding: .utf8)!)
'''], text=True))
assert len(screens) == 1, 'The MVP acceptance environment requires one connected display (#17)'
def fingerprints():
    return dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                source_sha256={str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
                               for path in sorted((root / 'src').glob('*.rs'))},
                driver_sha256={path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                               for path in sorted(scripts.iterdir()) if path.suffix in {'.py', '.swift'}})

identity = fingerprints()
manifest = dict(started=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                revision=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                dirty=bool(subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()),
                **identity,
                os=platform.platform(), screens=screens, cases=[])
recorder = out / 'record-window'
wrapper = out / 'with-japanese-ime'
subprocess.run(['xcrun', 'swiftc', '-parse-as-library', str(scripts / 'record_window.swift'), '-o', str(recorder)], check=True)
subprocess.run(['xcrun', 'swiftc', str(scripts / 'with_japanese_ime.swift'), '-o', str(wrapper)], check=True)
status = 0
for name in args.case or list(cases):
    assert fingerprints() == identity, 'The source, driver or product binary changed during acceptance'
    script, options = cases[name]
    command = [sys.executable, str(scripts / script), '--output', str(out / name), *options]
    if name in {'ime', 'ime-caption', 'presentation-ime'}:
        command.insert(0, str(wrapper))
    environment = dict(os.environ, PSYCHO_ACCEPTANCE_FULLSCREEN_WIDTH=str(int(screens[0]['width'])),
                       PSYCHO_ACCEPTANCE_LARGE_HEIGHT=str(min(992,int(screens[0]['height'])-64)))
    if name in {'keys', 'presentation-ime'}:
        environment['PSYCHO_ACCEPTANCE_RECORDER'] = str(recorder)
    entry = dict(case=name, command=command, expected='All runtime assertions pass',
                 started=datetime.datetime.now(datetime.timezone.utc).isoformat())
    print('RUN', name, flush=True)
    with (out / (name + '.log')).open('w') as log:
        owned_app = None
        case_exit = 0
        try:
            if name in {'assets', 'save-failure', 'conflict'}:
                subprocess.run([sys.executable, str(scripts / script), '--prepare'],
                               check=True, stdout=log, stderr=subprocess.STDOUT)
            if name in {'save-failure', 'conflict'}:
                fixture = root / 'target' / ('mvp-save-retry' if name == 'save-failure' else 'mvp-conflict-freeze') / 'presentation.kdl'
                owned_app = subprocess.Popen([str(binary), str(fixture)], stdout=log, stderr=subprocess.STDOUT)
                time.sleep(3)
                subprocess.run(['orca', 'computer', 'get-app-state', '--restore-window',
                                '--app', f'pid:{owned_app.pid}', '--json'], check=True, stdout=log, stderr=subprocess.STDOUT)
            result = subprocess.run(command, env=environment, stdout=log, stderr=subprocess.STDOUT)
            case_exit = result.returncode
        except subprocess.CalledProcessError as error:
            case_exit = error.returncode
        finally:
            if owned_app is not None:
                owned_app.terminate()
                try:
                    owned_app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    owned_app.kill()
                    owned_app.wait()
    if fingerprints() != identity:
        case_exit = 1
        entry['error'] = 'The source, driver or product binary changed during acceptance'
    entry.update(exit_code=case_exit, result='PASS' if case_exit == 0 else 'FAIL',
                 finished=datetime.datetime.now(datetime.timezone.utc).isoformat())
    manifest['cases'].append(entry)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    print(entry['result'], name, flush=True)
    if case_exit:
        status = case_exit
        break
sys.exit(status)
