import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--appearance', choices=['light', 'dark'], default='dark')
parser.add_argument('--expect-horizontal-blocked', action='store_true')
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(exist_ok=False, parents=True)
helpers = {}
for name, source in [('native', 'editor-shell/scroll_window.swift'),
                     ('menu', 'mvp-acceptance/edit_menu_state.swift'),
                     ('resize', 'mvp-acceptance/resize_window.swift')]:
    helpers[name] = out / name
    subprocess.run(['xcrun', 'swiftc', '-parse-as-library' if name == 'native' else '-g',
                    str(root / 'validation' / source), '-o', str(helpers[name])], check=True)
fixture = out / 'fixture.kdl'
fixture.write_text('presentation { metadata { title "Scroll"; }; slide id="first" { heading "Heading"; text "Original body"; }; }\n')
original = fixture.read_bytes()
records = []
with (out / 'product.log').open('w') as log:
    app = subprocess.Popen([str(root / 'target/debug/psycho'), str(fixture)],
        env=dict(os.environ, PSYCHO_EDITOR_STATE=str(out / 'state.json'),
                 PSYCHO_GLYPH_TRACE=str(out / 'glyphs.csv'),
                 PSYCHO_ACCEPTANCE_APPEARANCE=args.appearance), stdout=log, stderr=log)


def run(helper, *arguments):
    assert app.poll() is None
    subprocess.run([str(helpers[helper]), str(app.pid), *arguments],
                   check=True, stdout=subprocess.DEVNULL)


def menu(title):
    run('menu', '表示')
    run('menu', title)
    time.sleep(.3)


def observe(name):
    time.sleep(.5)
    run('native', 'capture', str(out / (name + '.png')))
    state = json.loads((out / 'state.json').read_text())
    rows = list(csv.reader((out / 'glyphs.csv').open()))
    state['lastGlyph'] = rows[-1]
    state['name'] = name
    state['kdlSha256'] = hashlib.sha256(fixture.read_bytes()).hexdigest()
    records.append(state)
    (out / 'observations.json').write_text(json.dumps(records, indent=2))
    return state


def invariant(state):
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft']:
        assert state[field] == before[field], (field, state)
    assert fixture.read_bytes() == original


def movement(a, b, dx, dy):
    invariant(b)
    ga, gb = a['lastGlyph'], b['lastGlyph']
    assert ga[:6] == gb[:6] and ga[8] == gb[8]
    for absolute, origin, delta in [(6, 9, dx), (7, 10, dy)]:
        assert abs(float(gb[origin]) - float(ga[origin]) - delta) < .01, (a, b)
        assert abs(float(gb[absolute]) - float(ga[absolute]) - delta) < .01, (a, b)


try:
    time.sleep(2)
    run('native', 'activate')
    time.sleep(.5)
    run('resize', '1440', '732')
    menu('100%表示')
    before = observe('before')
    assert before['actual'] and before['scale'] == 1
    assert before['dark'] == (args.appearance == 'dark')
    run('native', 'scroll', '0', '-240')
    horizontal = observe('horizontal')
    movement(before, horizontal, 0 if args.expect_horizontal_blocked else -240, 0)
    if not args.expect_horizontal_blocked:
        run('native', 'scroll', '160', '0')
        vertical = observe('vertical')
        movement(horizontal, vertical, 0, -160)
        for hidden in [False, True]:
            if hidden:
                menu('Navigatorを隠す')
                menu('Inspectorを隠す')
            menu('全体に合わせる')
            fitted = observe('fit-hidden' if hidden else 'fit-shown')
            invariant(fitted)
            scale = fitted['scale']
            assert abs(scale - min(fitted['canvasWidth'] / 1280, fitted['canvasHeight'] / 720)) < .0001
            origin = float(fitted['lastGlyph'][9])
            left = 8 if hidden else 176
            assert abs(origin - left - (fitted['canvasWidth'] - 1280 * scale) / 2) < 1
    (out / 'run.json').write_text(json.dumps(dict(revision=before['revision'],
        appearance=args.appearance, horizontalBlocked=args.expect_horizontal_blocked,
        binarySha256=hashlib.sha256((root / 'target/debug/psycho').read_bytes()).hexdigest(),
        result='PASS'), indent=2))
    print('PASS expected horizontal-scroll defect' if args.expect_horizontal_blocked
          else 'PASS native scroll movement and document invariants')
finally:
    app.terminate()
    app.wait(timeout=10)
