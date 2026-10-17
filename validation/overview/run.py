import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--revision', required=True)
parser.add_argument('--regressions', action='store_true')
parser.add_argument('--only', nargs='+')
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
steps = []
for section in ['main', 'guards', 'empty', 'invalid', 'representative', 'drag', 'resize', 'diagnostics']:
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
if args.only:
    unknown = set(args.only) - {name for name, _ in steps}
    if unknown:
        parser.error('Unknown steps: ' + ', '.join(sorted(unknown)))
    steps = [(name, command) for name, command in steps if name in args.only]
records = []
binary = root / 'target/debug/psycho'
binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
(out / 'binary.json').write_text(json.dumps(dict(sha256=binary_hash, revision=args.revision), indent=2))
for name, command in steps:
    if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_hash:
        raise RuntimeError('GUI binary changed during acceptance; build once before the suite')
    subprocess.run([str(root / '.agents/skills/verify/helpers/psycho-verify'), 'doctor'], check=True, cwd=root, stdout=subprocess.DEVNULL)
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
