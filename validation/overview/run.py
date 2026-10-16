import argparse
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--revision', required=True)
parser.add_argument('--regressions', action='store_true')
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
steps = []
for section in ['main', 'guards', 'empty', 'invalid', 'representative', 'drag', 'resize']:
    steps.append((section, [sys.executable, str(Path(__file__).with_name('acceptance.py')), '--section', section]))
for appearance in ['light', 'dark']:
    steps.append(('layout-' + appearance, [sys.executable, str(Path(__file__).with_name('acceptance.py')), '--section', 'layout', '--appearance', appearance]))
steps.append(('voiceover', [sys.executable, str(Path(__file__).with_name('voiceover.py'))]))
steps.append(('ime', ['xcrun', 'swift', str(root / 'validation/mvp-acceptance/with_japanese_ime.swift'), sys.executable, str(Path(__file__).with_name('ime.py'))]))
if args.regressions:
    for name, script, extra in [
        ('navigator', 'navigator/acceptance.py', []),
        ('navigator-light', 'navigator/layout.py', ['--appearance', 'light']),
        ('navigator-dark', 'navigator/layout.py', ['--appearance', 'dark']),
        ('slide-organization', 'slide-organization/acceptance.py', []),
        ('slide-guards', 'slide-organization/guards.py', []),
    ]:
        steps.append((name, [sys.executable, str(root / 'validation' / script), *extra]))
records = []
for name, command in steps:
    argv = command + ['--output', str(out / name), '--revision', args.revision]
    print('START ' + name, flush=True)
    with (out / (name + '.log')).open('w') as log:
        result = subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT, cwd=root)
    records.append(dict(name=name, command=argv, returncode=result.returncode))
    (out / 'results.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    print(('PASS ' if result.returncode == 0 else 'FAIL ') + name, flush=True)
    if name == 'voiceover':
        restoration = json.loads((out / name / 'restoration.json').read_text())
        assert not restoration['restored']['voiceOverEnabled'], restoration
        for key in ['welcome', 'scripting']:
            assert restoration['initial'][key] == restoration['restored'][key], restoration
raise SystemExit(int(any(record['returncode'] != 0 for record in records)))
