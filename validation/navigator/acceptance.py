import argparse
import collections
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from PIL import Image

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'validation/mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--appearance', choices=['light', 'dark'], default='dark')
parser.add_argument('--revision', required=True)
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor-state.json')
os.environ['PSYCHO_NAVIGATOR_TRACE'] = str(out / 'navigator.csv')
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = args.appearance
image = out / 'image.png'
Image.new('RGB', (200, 100), (220, 40, 40)).save(image)
long_heading = 'これは全文ツールチップで確認する長い日本語の見出しです'
slides = [
    'slide id="index-1" { heading "Heading 001"; text "Original body"; bullets { item "First item"; item "Second item"; }; }',
    f'slide {{ heading "{long_heading}"; code language="rust" "let value = 42;"; }}',
    'slide id="nested" { heading "Nested elements"; columns { column width=45 { heading "Left heading"; text "Left body"; bullets { item "Left item"; }; code language="rust" "let left = true;"; }; column width=55 { image "image.png" { caption "Nested caption"; }; text "Right body"; }; }; }',
    'slide id="blank" {}',
    'slide id="missing" { heading "Missing image"; image "missing.png" { caption "Missing caption"; }; }',
    'slide id="image" { heading "Reload image"; image "image.png" { caption "Image caption"; }; }',
]
slides += [f'slide id="slide-{n}" {{ heading "Heading {n:03}"; text "Body {n:03}"; }}' for n in range(7, 100)]
slides += ['slide id="last" { heading "Heading 100"; text "Tail original"; }']
source = 'presentation { metadata { title "Navigator acceptance"; };\n' + '\n'.join(slides) + '\n}\n'
(out / 'original.kdl').write_text(source)
gui = Gui(out, source)
records = []
key_helper = out / 'navigator-keys'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('navigator_keys.swift')), '-o', str(key_helper)], check=True)
scroll_helper = out / 'navigator-scroll'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('navigator_scroll.swift')), '-o', str(scroll_helper)], check=True)


def state():
    return json.loads((out / 'editor-state.json').read_text())


def observe(name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], out / f'{name}.png')
    record = dict(name=name, editor=state(), input=gui.state(), tree=snapshot['snapshot']['treeText'],
                  screenshotScale=snapshot['screenshot'].get('scale', 1), window=snapshot['snapshot']['window'])
    records.append(record)
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record


def invariant(before):
    after = state()
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft']:
        assert after[field] == before[field], (field, before, after)


def row(number):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) text (?:\(selected\) )?スライド ' + str(number) + r'(?: (.*?))?(?:, Secondary Actions:.*)?$', tree, re.MULTILINE)
    assert match, (number, tree)
    return match.group(1)


def select(number):
    gui.act('perform-secondary-action', '--element-index', row(number), '--action', 'pick', '--no-screenshot')
    assert state()['currentSlide'] == number - 1, state()
    assert state()['navigatorFocused'], state()


def button(label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) button ' + re.escape(label) + r'$', tree, re.MULTILINE)
    assert match, (label, tree)
    gui.act('click', '--element-index', match.group(1), '--no-screenshot')


def keys(direction, count):
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(key_helper), str(gui.app.pid), direction, str(count)], check=True)
    time.sleep(.5)
    gui.act('get-app-state')


def select_by_keys(number):
    current = state()['currentSlide'] + 1
    if number != current:
        keys('down' if number > current else 'up', abs(number - current))
    assert state()['currentSlide'] == number - 1, state()


def painted(text, scale_class):
    width = state()['navigatorWidth']
    thumbnail_scale = (width - 26) / 1280
    rows = list(csv.reader(gui.trace.open()))
    return [x for x in rows if len(x) == 12 and bytes.fromhex(x[3]).decode() == text and
            (abs(float(x[8]) - thumbnail_scale) < .00001) == (scale_class == 'thumbnail')]


def assert_both(text):
    assert painted(text, 'thumbnail'), ('thumbnail did not paint', text)
    assert painted(text, 'canvas'), ('canvas did not paint', text)


def image_rectangles(name, color):
    picture = Image.open(out / f'{name}.png').convert('RGB')
    scale = records[-1]['screenshotScale']
    width, height = picture.size
    rectangles = []
    for left, right in [(0, int(170 * scale)), (int(176 * scale), int(984 * scale))]:
        pixels = picture.load()
        def matches(pixel):
            red, green, blue = pixel
            return (red > 150 and green < 120 and blue < 120) if color[0] > color[2] else (red < 120 and green < 160 and blue > 180)
        points = [(x, y) for y in range(height) for x in range(left, min(right, width)) if matches(pixels[x, y])]
        assert points, (name, left, right, color)
        dominant = collections.Counter(pixels[x, y] for x, y in points).most_common(1)[0][0]
        points = [(x, y) for x, y in points if max(abs(pixels[x, y][i] - dominant[i]) for i in range(3)) < 8]
        bands = []
        for y in sorted({point[1] for point in points}):
            if not bands or y > bands[-1][-1] + 1:
                bands.append([])
            bands[-1].append(y)
        band = bands[-1]
        points = [point for point in points if band[0] <= point[1] <= band[-1]]
        bounds = [min(p[0] for p in points), min(p[1] for p in points), max(p[0] for p in points), max(p[1] for p in points)]
        rectangles.append(bounds)
    ratios = [(b[2] - b[0] + 1) / (b[3] - b[1] + 1) for b in rectangles]
    assert all(abs(ratio - 2) < .08 for ratio in ratios), ratios
    return dict(bounds=rectangles, ratios=ratios, expectedRatio=2, tolerance=.08)


try:
    before = state()
    assert before['revision'] == args.revision, before
    initial = observe('initial')
    assert long_heading in initial['tree'], initial['tree']
    assert re.search(r'text スライド 4, Secondary Actions:', initial['tree']), initial['tree']
    assert 'Missing image' in initial['tree']
    assert not painted('Heading 100', 'thumbnail')
    select(1)
    gui.key('Down')
    assert state()['currentSlide'] == 1 and state()['navigatorFocused']
    focus = observe('keyboard-focused')
    gui.key('Return')
    assert state()['canvasFocused'] and not state()['navigatorFocused']
    observe('return-canvas')
    invariant(before)
    select(4)
    observe('blank-slide')
    blank = Image.open(out / 'blank-slide.png').convert('RGB')
    scale = records[-1]['screenshotScale']
    for region in [(30, 530, 140, 580), (300, 350, 800, 550)]:
        pixels = blank.crop(tuple(int(value * scale) for value in region))
        assert all(min(pixel) > 245 for pixel in pixels.getdata()), region
    invariant(before)
    select(5)
    missing, recognized = gui.snapshot()
    shutil.copy(missing['screenshot']['path'], out / 'missing-placeholder.png')
    assert 'missing.png' in ''.join(item['text'] for item in recognized), recognized
    assert 'Missing caption' in ''.join(item['text'] for item in recognized), recognized
    invariant(before)
    select(1)
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(scroll_helper), str(gui.app.pid), '16000'], check=True)
    end = observe('manual-scroll-end')
    assert 'スライド 100 Heading 100' in end['tree'], end['tree']
    assert state()['currentSlide'] == 0
    gui.key('Tab')
    gui.key('Tab')
    end_again = observe('manual-scroll-stays')
    assert 'スライド 100 Heading 100' in end_again['tree'], end_again['tree']
    invariant(before)
    select(100)
    observe('last-selected')
    invariant(before)
    assert gui.fixture.read_bytes() == gui.original
    assert_both('Tail original')
    gui.edit('Tail original', 28)
    gui.replace('Tail updated')
    observe('draft-update')
    assert_both('Tail updated')
    assert state()['draft']
    gui.hotkey('CmdOrCtrl+S')
    assert b'text "Tail updated"' in gui.fixture.read_bytes()
    select(100)
    gui.trace.write_text('')
    gui.press('Undo')
    observe('undo-update')
    assert 'Tail original' in bytes.fromhex(state()['sourceHex']).decode()
    assert_both('Tail original')
    gui.trace.write_text('')
    gui.press('Redo')
    observe('redo-update')
    assert 'Tail updated' in bytes.fromhex(state()['sourceHex']).decode()
    assert_both('Tail updated')
    gui.hotkey('CmdOrCtrl+S')
    saved = gui.fixture.read_text()
    gui.trace.write_text('')
    gui.fixture.write_text(saved.replace('Tail updated', 'External tail'))
    time.sleep(1.5)
    observe('external-reload')
    assert_both('External tail')
    assert 'External tail' in bytes.fromhex(state()['sourceHex']).decode()
    assert not state()['dirty']
    select(100)
    select_by_keys(6)
    gui.trace.write_text('')
    image_before = observe('image-before')
    before_pixels = image_rectangles('image-before', (220, 40, 40))
    gui.outline_select('Element 2 · 画像')
    Image.new('RGB', (200, 100), (40, 80, 220)).save(image)
    button('画像を再読み込み')
    image_after = observe('image-after')
    after_pixels = image_rectangles('image-after', (40, 80, 220))
    (out / 'image-ratios.json').write_text(json.dumps(dict(before=before_pixels, after=after_pixels), indent=2))
    assert not state()['dirty'] and not state()['undo']
    select(6)
    gui.trace.write_text('')
    select_by_keys(3)
    observe('nested-elements')
    for text in ['Left heading', 'Left body', 'Left item', 'let left = true;', 'Nested caption', 'Right body']:
        assert_both(text)
    positions = collections.defaultdict(lambda: collections.defaultdict(list))
    for x in csv.reader(gui.trace.open()):
        if len(x) != 12:
            continue
        text = bytes.fromhex(x[3]).decode()
        if text not in ['Nested elements', 'Left heading', 'Left body', 'Left item', 'let left = true;', 'Nested caption', 'Right body']:
            continue
        key = (text, x[2], x[4], x[5])
        positions[key][float(x[8])].append(((float(x[6]) - float(x[9])) / float(x[8]), (float(x[7]) - float(x[10])) / float(x[8])))
    errors = []
    for key, scaled in positions.items():
        assert len(scaled) >= 2, (key, scaled)
        values = [point for points in scaled.values() for point in points]
        errors.append(max(max(point[axis] for point in values) - min(point[axis] for point in values) for axis in (0, 1)))
    assert errors and max(errors) < .003, max(errors)
    (out / 'glyph-comparison.json').write_text(json.dumps(dict(glyphs=len(positions), maxBasePositionError=max(errors), tolerance=.003), indent=2))
    gui.outline_select('Element 2 · 2列')
    gui.outline_select('2列 2 · 左列 · 見出し 1')
    observe('nested-selected-no-thumbnail-border')
    select(3)
    select_by_keys(1)
    observe('keyboard-return-first')
    assert state()['navigatorFocused']
    assert 'スライド 1 Heading 001' in records[-1]['tree']
    gui.edit('Original body', 28)
    gui.key('Down')
    assert state()['currentSlide'] == 0 and gui.state()['focused']
    gui.key('Return')
    assert state()['currentSlide'] == 0 and gui.state()['focused']
    gui.hotkey('CmdOrCtrl+Z')
    gui.click(80, 180)
    last_good = state()['sourceHex']
    gui.trace.write_text('')
    gui.fixture.write_text('presentation { slide { heading 42; }; }\n')
    time.sleep(1.5)
    invalid = observe('external-invalid')
    assert state()['sourceHex'] == last_good
    assert 'スライド 1 Heading 001' in invalid['tree']
    assert_both('Original body')
    ranges = [list(map(float, x)) for x in csv.reader((out / 'navigator.csv').open())]
    assert max(int(x[1] - x[0]) for x in ranges) < 15, ranges
    assert any(int(x[1]) == 100 and int(x[0]) > 80 for x in ranges), ranges
    (out / 'visible-ranges.json').write_text(json.dumps(dict(maxRowsPerRequest=max(int(x[1] - x[0]) for x in ranges), reaches100=True, requests=len(ranges)), indent=2))
    print('PASS 100-slide native navigation, live edit/history/reload/images, normalized glyphs and document invariants', flush=True)
except Exception:
    observe('failure')
    raise
finally:
    gui.stop()

frozen_out = out / 'frozen-draft'
os.environ['PSYCHO_EDITOR_STATE'] = str(frozen_out / 'editor-state.json')
os.environ['PSYCHO_NAVIGATOR_TRACE'] = str(frozen_out / 'navigator.csv')
frozen_gui = Gui(frozen_out, source)
try:
    frozen_gui.edit('Original body', 28)
    frozen_gui.replace('Frozen draft')
    frozen_input = frozen_gui.state()['text']
    frozen_gui.fixture.write_text(source.replace('Original body', 'External body'))
    time.sleep(1.5)
    before = json.loads((frozen_out / 'editor-state.json').read_text())
    assert before['draft'] and frozen_gui.state()['text'] == frozen_input
    snapshot = frozen_gui.act('get-app-state')
    tree = snapshot['snapshot']['treeText']
    assert 'button 外部変更を読み込む' in tree and 'button (disabled) 発表' in tree, tree
    match = re.search(r'^\s*(\d+) text スライド 2 ', tree, re.MULTILINE)
    assert match, tree
    frozen_gui.act('perform-secondary-action', '--element-index', match.group(1), '--action', 'pick', '--no-screenshot')
    after = json.loads((frozen_out / 'editor-state.json').read_text())
    for field in ['sourceHex', 'dirty', 'undo', 'redo', 'draft']:
        assert before[field] == after[field], field
    assert after['currentSlide'] == 1 and after['navigatorFocused']
    assert frozen_gui.state()['text'] == frozen_input
    snapshot = frozen_gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], frozen_out / 'frozen-navigation.png')
    (frozen_out / 'observations.json').write_text(json.dumps(dict(before=before, after=after, snapshot=snapshot), ensure_ascii=False, indent=2))
    print('PASS conflict navigation preserves frozen draft and document history', flush=True)
finally:
    frozen_gui.stop()

invalid_out = out / 'initial-invalid'
os.environ['PSYCHO_EDITOR_STATE'] = str(invalid_out / 'editor-state.json')
os.environ['PSYCHO_NAVIGATOR_TRACE'] = str(invalid_out / 'navigator.csv')
invalid_gui = Gui(invalid_out, 'presentation { slide { heading 42; }; }\n')
try:
    snapshot = invalid_gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], invalid_out / 'invalid.png')
    (invalid_out / 'invalid.json').write_text(json.dumps(snapshot, ensure_ascii=False, indent=2))
    assert not re.search(r'text (?:\(selected\) )?スライド \d', snapshot['snapshot']['treeText'])
    assert not (invalid_out / 'navigator.csv').exists()
    assert invalid_gui.fixture.read_bytes() == invalid_gui.original
    print('PASS initial invalid source has no partial thumbnails', flush=True)
finally:
    invalid_gui.stop()

(out / 'run.json').write_text(json.dumps(dict(revision=args.revision, appearance=args.appearance,
        os=subprocess.check_output(['sw_vers'], text=True), binarySha256=hashlib.sha256((root / 'target/debug/psycho').read_bytes()).hexdigest(), result='PASS'), indent=2))
