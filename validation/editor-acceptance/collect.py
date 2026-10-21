#!/usr/bin/env python3
"""Archive selected issue-56 evidence without deriving acceptance verdicts."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path


TEXT = {'.json', '.jsonl', '.log', '.csv', '.txt'}
IMAGES = {'.png', '.jpg', '.jpeg', '.webp'}
STRUCTURED = {
    'results.json', 'result.json', 'run.json', 'restoration.json',
    'native-input-build.json', 'native-input.jsonl', 'editor-state.json',
    'state.json', 'binary.json', 'image-ratios.json', 'visible-ranges.json',
    'voiceover-diagnostics.json', 'speech.json', 'observation-recovery.jsonl',
    'editor.json', 'keyboard-transitions.json', 'activated-native.json',
    'tail-native.json', 'after-burst.json',
}
TRACES = {
    'glyph-origins.csv', 'presentation-glyphs.csv',
    'layout-glyph-comparison.csv', 'layout-raster-comparison.csv',
    'presentation-input.csv',
}
ROOT = {
    'issue.json', 'spec.json', 'build-provenance.json', 'displays.txt',
    'build.log', 'baseline-build.log', 'baseline-tests.log', 'integration.log',
    'integrated-tests.log', 'integration-repair-tests.log',
    'overview-suite.log', 'workflow-suite.log', 'final-suite.log', 'accepted-suite.log',
    'layout-readonly-review.log',
    'recheck-suite.log',
    'confirm-suite.log', 'keyboard-voiceover-suite.log',
    'voiceover-final-suite.log',
}
SKIP = {'env', 'venv', '.venv', '__pycache__', 'node_modules'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('target/issue-56'))
    parser.add_argument('--output', type=Path, default=Path('validation/editor-acceptance/evidence'))
    parser.add_argument('--runs', nargs='+', default=[
        'overview', 'workflows', 'final', 'accepted', 'recheck', 'confirm', 'keyboard-voiceover',
        'voiceover-final',
    ])
    parser.add_argument('--images', nargs='*', default=[], metavar='CASE')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    source, output = args.source.resolve(), args.output.resolve()
    if not source.is_dir() or not source.is_relative_to(repo / 'target'):
        parser.error('--source must be a directory under this repository target/')
    if output.exists() or output.is_relative_to(source):
        parser.error('--output must be fresh and outside --source')
    prefixes = [Path(value) for value in args.images]
    if any(p.is_absolute() or '..' in p.parts or len(p.parts) < 2 for p in prefixes):
        parser.error('--images needs relative stage/case directory prefixes')
    if any(Path(run).name != run or run in {'.', '..'} for run in args.runs):
        parser.error('--runs needs directory names')
    files = {p for p in source.iterdir() if p.is_file() and p.name in ROOT}
    probes = {p for p in source.glob('*probe*.log') if p.is_file()}
    files.update(probes)
    runs = []
    for stage in args.runs:
        results = source / stage / 'results.json'
        if results.is_file() and not results.is_symlink() and results.resolve().is_relative_to(source):
            records = json.loads(results.read_text())
            if not isinstance(records, list) or any(not isinstance(r, dict) for r in records):
                parser.error(f'{results} must contain an array of result records')
            runs.extend({'stage': stage, 'result': record} for record in records)
    for stage in dict.fromkeys([*args.runs, 'baseline']):
        stage_path = source / stage
        if stage_path.is_symlink():
            continue
        for directory, folders, names in os.walk(stage_path, followlinks=False):
            folders[:] = sorted(f for f in folders if f not in SKIP and not (Path(directory) / f).is_symlink())
            for name in names:
                path = Path(directory) / name
                relative = path.relative_to(source)
                selected_image = any(relative.is_relative_to(p) for p in prefixes)
                asset = 'assets' in relative.parts
                structured = (name in STRUCTURED or name.endswith(('-observations.json', '-comparison.json'))
                              or name == 'observations.json' or name.startswith('launch-') and path.suffix == '.json')
                selected_snapshot = selected_image and path.suffix == '.json' and path.with_suffix('.png').is_file()
                if (path.suffix == '.kdl' or name in TRACES or structured or selected_snapshot
                        or path.suffix == '.log' and len(relative.parts) == 2
                        or path.suffix in IMAGES and (selected_image or asset)):
                    files.add(path)
    files = sorted(p for p in files if not p.is_symlink()
                   and p.resolve().is_relative_to(source) and not p.stat().st_mode & 0o111)
    output.mkdir(parents=True, exist_ok=False)
    manifest, checksums = [], []
    for path in files:
        raw = path.read_bytes()
        data = raw
        transformation = 'identity'
        if path in probes:
            data = b''.join(line for line in raw.splitlines(keepends=True) if line.startswith(b'PASS'))
            transformation = 'PASS-prefixed lines only'
        relative = path.relative_to(source)
        if path.suffix in TEXT:
            data = gzip.compress(data, mtime=0)
            relative = relative.with_name(relative.name + '.gz')
            transformation += '; gzip mtime=0'
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        manifest.append({'source': path.relative_to(source).as_posix(), 'raw_sha256': digest(raw),
                         'archive': relative.as_posix(), 'archive_sha256': digest(data),
                         'bytes': len(raw), 'archive_bytes': len(data), 'transformation': transformation})
        checksums.append((relative.as_posix(), digest(data)))
    for name, value in [('manifest.json', manifest), ('runs.json', runs)]:
        data = (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()
        (output / name).write_bytes(data)
        checksums.append((name, digest(data)))
    (output / 'SHA256SUMS').write_text(''.join(f'{sha}  {name}\n' for name, sha in sorted(checksums)))
    print(f'Archived {len(manifest)} source artifacts and {len(runs)} executed result records to {output}')


if __name__ == '__main__':
    main()
