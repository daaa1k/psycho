"""Drive real Kotoeri keys and inspect callback values and screenshots at each Return.

Start the probe with PSYCHO_IME_TRACE_DIR pointing to target/ime-trace.
Requires a fresh small editor window, Kotoeri Romaji, and the OCR executable
compiled from recognize_heading.swift. No input-source or window-manager changes.
"""
import argparse
import csv
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--app', required=True)
parser.add_argument('--window-id', required=True)
parser.add_argument('--orca', default='orca')
parser.add_argument('--ocr', required=True)
parser.add_argument('--iterations', type=int, default=100)
parser.add_argument('--output', type=Path, default=ROOT / 'target/ime-loop')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
trace = ROOT / 'target/ime-trace/ime-events.tsv'
painted = ROOT / 'target/ime-trace/ime-painted.txt'


def action(*arguments, screenshot=False):
    command = [args.orca, 'computer', *arguments, '--app', args.app,
               '--window-id', args.window_id, '--json']
    if not screenshot:
        command.append('--no-screenshot')
    result = json.loads(subprocess.check_output(command))
    if not result.get('ok'):
        raise RuntimeError(result)
    return result['result']


def check(iteration, step, start):
    result = action('get-app-state', screenshot=True)
    filename = args.output / f'{iteration:03}-{step}.png'
    shutil.copyfile(result['screenshot']['path'], filename)
    observed = subprocess.check_output([args.ocr, str(filename)], text=True).strip()
    value = painted.read_text()
    events = trace.read_text().splitlines()[start:]
    contents = [bytes.fromhex(row.split('\t')[1]).decode() for row in events]
    if any('日本語変換日本語変換' in text for text in [observed, value, *contents]):
        raise AssertionError(f'duplicate at {iteration}/{step}: {observed!r}, {value!r}')
    if value != '日本語変換' or observed != '日本語変換':
        raise AssertionError(f'missing/unexpected at {iteration}/{step}: {observed!r}, {value!r}')
    writer.writerow([iteration, step, value, observed, len(events)])
    output.flush()


with (args.output / 'checks.csv').open('w', newline='') as output:
    writer = csv.writer(output, lineterminator='\n')
    writer.writerow(['iteration', 'step', 'painted_value', 'heading_ocr', 'events'])
    for iteration in range(1, args.iterations + 1):
        action('click', '--x', '500', '--y', '83')  # enter title editing
        action('hotkey', '--key', 'CmdOrCtrl+A')
        start = len(trace.read_text().splitlines())
        for key in 'nihongohenkann':
            action('press-key', '--key', key)
        action('press-key', '--key', 'Space')
        for step in ('return-1', 'return-2'):
            action('press-key', '--key', 'Return')
            check(iteration, step, start)
        if iteration % 10 == 0:
            action('hotkey', '--key', 'CmdOrCtrl+A')
            action('hotkey', '--key', 'Ctrl+Shift+R')
            action('press-key', '--key', 'Return')
            action('press-key', '--key', 'Return')
            check(iteration, 'reconvert', start)
        action('click', '--x', '32', '--y', '124')
        saved = (ROOT / 'target/PROTOTYPE-title.txt').read_text()
        assert saved == '日本語変換', repr(saved)
        print(f'PASS {iteration}: both Returns, callbacks, OCR, save', flush=True)
