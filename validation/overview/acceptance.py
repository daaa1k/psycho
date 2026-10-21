import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'validation/mvp-acceptance'))
from gui_session import Gui
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--revision', required=True)
parser.add_argument('--section', choices=['main', 'layout', 'guards', 'empty', 'invalid', 'representative', 'drag', 'resize', 'diagnostics'], default='main')
parser.add_argument('--appearance', choices=['light', 'dark'], default='light')
args = parser.parse_args()
out = args.output.resolve()
assert out.is_relative_to((root / 'target').resolve())
out.mkdir(parents=True, exist_ok=False)
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor.json')
os.environ['PSYCHO_COLLECTION_TRACE'] = str(out / 'collections.csv')
os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = args.appearance
records = []


def source(count=100):
    slides = []
    for n in range(1, count + 1):
        body = f'heading "Heading {n:03}"; text #"raw {n}"#;'
        if args.section == 'layout':
            if n == 4:
                body = ''
            elif n == 5:
                body = 'heading "これは全文ツールチップで確認する長い日本語の見出しです。全体の構成を確認して編集へ戻ります。";'
            elif n == 6:
                body = 'heading "Nested columns"; columns { column width=45 { bullets { item "First"; item "Second"; }; }; column width=55 { code language="rust" "fn main() {}"; }; };'
            elif n == 7:
                body = 'heading "Missing image"; image "missing-overview.png" { caption "Missing asset remains identifiable"; };'
        slides.append(f' slide id="s{n}" {{ {body} }} // trailing {n}\n')
    return 'presentation {\n metadata { title "Overview acceptance" }\n // standalone comment\n' + ''.join(slides) + '}\n'


def compile_helper(name, path):
    target = out / name
    subprocess.run(['xcrun', 'swiftc', str(path), '-o', str(target)], check=True)
    return target


keys = compile_helper('keys', Path(__file__).with_name('keys.swift'))
pointer_bin = compile_helper('pointer', root / 'validation/slide-organization/pointer.swift')
resize = compile_helper('resize', root / 'validation/mvp-acceptance/resize_window.swift')
native_double_click = compile_helper('native-double-click', root / 'validation/mvp-acceptance/native_double_click.swift')


def state():
    return json.loads((out / 'editor.json').read_text())


def text():
    return bytes.fromhex(state()['sourceHex']).decode()


def observe(gui, name):
    snapshot = gui.act('get-app-state')
    shutil.copy(snapshot['screenshot']['path'], out / f'{name}.png')
    value = state()
    assert value['revision'] == args.revision, value['revision']
    record = dict(name=name, editor=value, tree=snapshot['snapshot']['treeText'],
                  window=snapshot['snapshot']['window'], scale=snapshot['screenshot'].get('scale', 1))
    records.append(record)
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return record


def unchanged(before, fields=('sourceHex', 'dirty', 'undo', 'redo', 'draft', 'currentSlide')):
    after = state()
    for field in fields:
        if field == 'sourceHex' and after[field] != before[field]:
            expected = bytes.fromhex(before[field])
            actual = bytes.fromhex(after[field])
            (out / 'invariant-expected.kdl').write_bytes(expected)
            (out / 'invariant-actual.kdl').write_bytes(actual)
            raise AssertionError((field, hashlib.sha256(expected).hexdigest(), hashlib.sha256(actual).hexdigest()))
        assert after[field] == before[field], (field, before[field], after[field])


def button(gui, label):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    matches = re.findall(r'^\s*(\d+) button ' + re.escape(label) + '$', tree, re.M)
    assert matches, (label, tree)
    gui.act('click', '--element-index', matches[-1], '--no-screenshot')


def pick(gui, number, overview=True):
    tree = gui.act('get-app-state')['snapshot']['treeText']
    if overview:
        tree = tree.split('list Overviewのスライド一覧', 1)[1]
    index = re.search(r'^\s*(\d+) text (?:\([^)]*\) )?スライド ' + str(number) + r'(?!\d)(?: |,)', tree, re.M)
    assert index, (number, tree)
    gui.act('perform-secondary-action', '--element-index', index[1], '--action', 'pick', '--no-screenshot')
    assert state()['overviewFocused' if overview else 'navigatorFocused']


def repeat(gui, key, count):
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(keys), str(gui.app.pid), key, str(count)], check=True)
    time.sleep(.3)


def pointer(gui, kind, x, y):
    subprocess.run([str(pointer_bin), str(gui.app.pid), kind, str(x), str(y)], check=True)
    if kind.endswith('up'):
        gui.act('get-app-state')


def drag(gui, start, end):
    pointer(gui, 'down', *start)
    pointer(gui, 'drag', *end)
    time.sleep(.2)


def ids():
    return re.findall(r'slide id="([^"]+)"', text())


def layout(gui):
    gui.menu('表示', 'Overview')
    original = state()
    assert original['dark'] == (args.appearance == 'dark')
    for width, height in [(1024, 700), (1280, 860), (1440, 960)]:
        subprocess.run([str(resize), str(gui.app.pid), str(width), str(height + 32)], check=True)
        time.sleep(.5)
        pick(gui, 1)
        record = observe(gui, f'{width}x{height}-first')
        columns = state()['overviewColumns']
        assert columns == int(state()['canvasWidth'] // 240), state()
        gui.key('Right')
        assert state()['currentSlide'] == 1
        gui.key('Up')
        assert state()['currentSlide'] == 1
        gui.key('Down')
        assert state()['currentSlide'] == 1 + columns
        repeat(gui, 'right', 100)
        tail = observe(gui, f'{width}x{height}-tail')
        assert state()['currentSlide'] == 99 and 'スライド 100 Heading 100' in tail['tree']
        gui.key('Down')
        assert state()['currentSlide'] == 99
        repeat(gui, 'left', 100)
        assert state()['currentSlide'] == 0
    for label in ['Navigatorを隠す', 'Navigatorを表示', 'Navigatorを広くする', 'Navigatorを狭くする']:
        gui.menu('表示', label)
        observe(gui, label)
    unchanged(original, ('sourceHex', 'dirty', 'undo', 'redo', 'draft'))


def main(gui):
    gui.menu('表示', 'Inspectorを広くする')
    original = observe(gui, 'edit-initial')['editor']
    gui.menu('表示', 'Overview')
    overview = observe(gui, 'overview-native')['editor']
    unchanged(original)
    assert overview['overview'] and overview['inspectorWidth'] == 0
    assert overview['inspector'] == original['inspector'] and overview['overviewFocused']
    button(gui, '表示')
    popup = gui.act('get-app-state')['snapshot']['treeText']
    assert 'button (disabled) Inspectorの表示切り替え' in popup
    button(gui, '編集')
    edit = observe(gui, 'edit-popup')['editor']
    unchanged(original)
    assert not edit['overview'] and edit['inspectorWidth'] == original['inspectorWidth']
    gui.menu('表示', 'Inspectorを隠す')
    hidden = observe(gui, 'inspector-hidden-edit')['editor']
    unchanged(original)
    assert not hidden['inspector'] and hidden['inspectorWidth'] == 0
    gui.menu('表示', 'Overview')
    hidden_overview = observe(gui, 'inspector-hidden-overview')['editor']
    unchanged(hidden)
    assert hidden_overview['overview'] and not hidden_overview['inspector']
    assert hidden_overview['inspectorWidth'] == 0
    gui.menu('表示', '編集')
    hidden_edit = observe(gui, 'inspector-hidden-restored-edit')['editor']
    unchanged(hidden)
    assert not hidden_edit['overview'] and not hidden_edit['inspector']
    assert hidden_edit['inspectorWidth'] == 0
    gui.menu('表示', 'Inspectorを表示')
    restored = observe(gui, 'inspector-visible-restored-edit')['editor']
    unchanged(original)
    assert restored['inspector'] and restored['inspectorWidth'] == original['inspectorWidth']
    button(gui, '表示')
    button(gui, 'Overview')
    pick(gui, 2)
    gui.key('Return')
    assert not state()['overview'] and state()['currentSlide'] == 1 and state()['canvasFocused']
    gui.menu('表示', 'Overview')
    pick(gui, 1, overview=False)
    gui.key('Return')
    assert not state()['overview'] and state()['canvasFocused']
    gui.menu('表示', 'Overview')
    cell = state()['canvasWidth'] / state()['overviewColumns']
    gui.click(176 + cell * 1.4, 220, count=2)
    assert not state()['overview'] and state()['currentSlide'] == 1
    gui.menu('表示', 'Overview')
    pick(gui, 1)
    before = state()
    width = state()['canvasWidth']
    cell = width / state()['overviewColumns']
    left = 176
    start = (left + cell * .3, 220)
    noop = (left + cell * .35, 220)
    destination = (left + cell * 1.8, 220)
    for name, end, cancel in [('noop', noop, False), ('escape', destination, True), ('outside', (50, 220), False)]:
        drag(gui, start, end)
        observe(gui, name + '-drag')
        if cancel:
            gui.key('Escape')
        pointer(gui, 'up', *end)
        observe(gui, name + '-drop')
        unchanged(before)
    drag(gui, start, destination)
    observe(gui, 'insertion-horizontal')
    pointer(gui, 'up', *destination)
    moved = observe(gui, 'moved-once')['editor']
    assert ids()[:3] == ['s2', 's1', 's3'] and moved['currentSlide'] == 1 and moved['overviewFocused']
    moved_source = text()
    (out / 'moved.kdl').write_text(moved_source)
    gui.menu('編集', '元に戻す')
    assert text() == source() and state()['currentSlide'] == 0 and state()['overviewFocused']
    gui.menu('編集', 'やり直す')
    assert text() == moved_source and state()['currentSlide'] == 1 and state()['overviewFocused']
    gui.menu('編集', '元に戻す')
    pick(gui, 1)
    row_height = (cell - 26) * 9 / 16 + 42
    across = (left + cell * 1.8, 220 + row_height)
    drag(gui, start, across)
    observe(gui, 'insertion-across-row')
    pointer(gui, 'up', *across)
    assert ids()[state()['overviewColumns'] + 1] == 's1'
    gui.menu('編集', '元に戻す')
    assert text() == source()
    pick(gui, 1)
    pointer(gui, 'right', *start)
    pointer(gui, 'right-up', *start)
    button(gui, '後ろへ移動')
    assert ids()[:3] == ['s2', 's1', 's3'] and state()['overviewFocused']
    gui.menu('編集', '元に戻す')
    pick(gui, 1)
    gui.hotkey('CmdOrCtrl+Down')
    assert ids()[:3] == ['s2', 's1', 's3'] and state()['overviewFocused']
    gui.menu('編集', '元に戻す')
    pick(gui, 1)
    gui.hotkey('CmdOrCtrl+Shift+N')
    assert len(re.findall(r'\bslide\b', text())) == 101 and state()['currentSlide'] == 1 and state()['overviewFocused']
    gui.key('Backspace')
    assert len(re.findall(r'\bslide\b', text())) == 100 and state()['overviewFocused']
    gui.menu('編集', '元に戻す')
    gui.menu('編集', '元に戻す')
    assert text() == source()
    pick(gui, 1)
    gui.menu('表示', '編集')
    assert state()['canvasFocused']
    drag(gui, (80, 180), (80, 340))
    pointer(gui, 'up', 80, 340)
    observe(gui, 'navigator-drag-from-canvas-focus')
    assert ids()[:3] == ['s2', 's1', 's3'] and state()['navigatorFocused']
    gui.menu('編集', '元に戻す')
    assert text() == source() and state()['navigatorFocused']
    gui.menu('編集', 'やり直す')
    assert ids()[:3] == ['s2', 's1', 's3'] and state()['navigatorFocused']
    gui.key('Down')
    assert state()['currentSlide'] == 2 and state()['navigatorFocused']
    gui.menu('編集', '元に戻す')
    pick(gui, 1, overview=False)
    gui.menu('表示', 'Overview')
    pick(gui, 1)
    drag(gui, start, destination)
    pointer(gui, 'up', *destination)
    gui.key('Return')
    assert not state()['overview'] and state()['currentSlide'] == 1
    gui.outline_select('Element 1 · 見出し')
    observe(gui, 'direct-heading-before')
    gui.edit('Heading 001', 48)
    observe(gui, 'direct-heading-after')
    gui.replace('Edited from Overview')
    gui.menu('ファイル', '保存')
    saved = gui.fixture.read_text()
    assert 'heading "Edited from Overview"' in saved and 'id="s1"' in saved
    observe(gui, 'edited-saved')
    gui.menu('プレゼンテーション', '現在からプレゼンテーション')
    assert state()['presenting']
    time.sleep(1.2)
    observe(gui, 'present-current')
    subprocess.run([str(keys), str(gui.app.pid), 'escape', '1'], check=True)
    time.sleep(1.2)
    assert not state()['presenting'] and state()['currentSlide'] == 1
    gui.menu('表示', 'Overview')
    assert 'スライド 2 Edited from Overview' in observe(gui, 'returned-same-slide')['tree']
    assert gui.fixture.read_text() == saved
    (out / 'saved.kdl').write_text(saved)


def guards(gui):
    gui.outline_select('Element 1 · 2列')
    button(gui, '左列幅を数値入力')
    gui.replace('0')
    invalid = observe(gui, 'invalid-width')['editor']
    invalid_input = gui.state()
    gui.menu('表示', 'Overview')
    unchanged(invalid)
    assert not state()['overview'] and gui.state()['text'] == '0'
    assert gui.state()['focused'] == invalid_input['focused']
    button(gui, '表示')
    assert not state()['overview'] and gui.state()['text'] == '0'
    observe(gui, 'invalid-popup-rejected')
    gui.replace('60')
    gui.menu('表示', 'Overview')
    assert state()['overview'] and 'column width=60' in text()
    gui.menu('表示', '編集')
    button(gui, 'Presentation title')
    snapshot, rows = gui.snapshot('title-input')
    matches = [row for row in rows if row['text'] == 'Overview guards'
               and row['x'] > snapshot['snapshot']['window']['width'] - 300]
    assert len(matches) == 1, rows
    subprocess.run([str(native_double_click), str(gui.app.pid),
                    str(matches[0]['x']), str(matches[0]['y'])], check=True)
    gui.replace('')
    invalid_title = observe(gui, 'invalid-title')['editor']
    gui.menu('表示', 'Overview')
    unchanged(invalid_title)
    assert not state()['overview'] and gui.state()['text'] == ''
    gui.replace('Pending title')
    pending = state()
    gui.fixture.write_text(gui.fixture.read_text().replace('Overview guards', 'External title'))
    time.sleep(1.2)
    conflict = observe(gui, 'external-frozen-draft')['editor']
    assert conflict['draft']
    frozen_input = gui.state()
    gui.menu('表示', 'Overview')
    unchanged(conflict)
    assert gui.state() == frozen_input
    assert not state()['overview'] and gui.state()['text'] == 'Pending title'
    observe(gui, 'frozen-draft-switch-rejected')
    assert '競合' in gui.text('frozen-reason')


def drag_edges(gui):
    subprocess.run([str(resize), str(gui.app.pid), '1024', '732'], check=True)
    gui.menu('表示', 'Overview')
    pick(gui, 1)
    record = observe(gui, 'edge-initial')
    width = state()['canvasWidth']
    columns = state()['overviewColumns']
    assert columns == 3
    cell = width / columns
    row_height = (cell - 26) * 9 / 16 + 42
    bottom = record['window']['height'] - 75
    start = (176 + cell * .3, 220)
    tail = (176 + cell * .9, bottom)
    drag(gui, start, tail)
    time.sleep(10)
    edge = observe(gui, 'stationary-bottom-partial-row')
    assert 'スライド 100 Heading 100' in edge['tree']
    pointer(gui, 'up', *tail)
    assert ids()[-1] == 's1' and state()['currentSlide'] == 99
    moved = text()
    observe(gui, 'stationary-bottom-moved')
    gui.menu('編集', '元に戻す')
    assert text() == source() and state()['currentSlide'] == 0
    gui.menu('編集', 'やり直す')
    assert text() == moved and state()['currentSlide'] == 99
    gui.menu('編集', '元に戻す')
    repeat(gui, 'right', 100)
    assert state()['currentSlide'] == 99
    last = (176 + cell * .3, bottom - row_height * .4)
    head = (176 + cell * .1, 165)
    drag(gui, last, head)
    time.sleep(10)
    edge = observe(gui, 'stationary-top')
    assert 'スライド 1 Heading 001' in edge['tree']
    pointer(gui, 'up', *head)
    assert ids()[0] == 's100' and state()['currentSlide'] == 0
    gui.menu('編集', '元に戻す')
    assert text() == source()
    repeat(gui, 'left', 100)
    pick(gui, 1)
    drag(gui, start, (176 + cell * 1.8, 220))
    gui.menu('表示', '編集')
    pointer(gui, 'up', 176 + cell * 1.8, 220)
    assert text() == source() and not state()['overview']
    observe(gui, 'mode-switch-cancels-drag')
    gui.menu('表示', 'Overview')
    pick(gui, 1)
    drag(gui, start, (176 + cell * 1.8, 220))
    updated = source().replace('Overview acceptance', 'Externally reloaded')
    gui.fixture.write_text(updated)
    time.sleep(1.2)
    pointer(gui, 'up', 176 + cell * 1.8, 220)
    assert text() == updated and not state()['undo']
    observe(gui, 'stale-source-drag-canceled')


def resize_drag(gui):
    gui.menu('表示', 'Overview')
    pick(gui, 1)
    cell = state()['canvasWidth'] / state()['overviewColumns']
    before = state()
    start = (176 + cell * .3, 220)
    end = (176 + cell * 1.8, 220)
    drag(gui, start, end)
    observe(gui, 'before-height-resize')
    subprocess.run([str(resize), str(gui.app.pid), '1280', '992'], check=True)
    time.sleep(.5)
    pointer(gui, 'up', *end)
    observe(gui, 'after-height-resize-drop')
    unchanged(before)


def empty(gui):
    gui.menu('表示', 'Overview')
    before = observe(gui, 'empty-overview')['editor']
    gui.key('Return')
    assert state()['overview']
    button(gui, 'スライド追加')
    added = observe(gui, 'empty-added-first')['editor']
    assert 'slide' in text() and state()['currentSlide'] == 0
    gui.menu('スライド', 'スライドを削除')
    assert 'slide' not in text()
    gui.menu('編集', '元に戻す')
    assert state()['sourceHex'] == added['sourceHex']
    gui.menu('編集', '元に戻す')
    assert state()['sourceHex'] == before['sourceHex']
    observe(gui, 'empty-history-restored')


def invalid(gui):
    before = state()
    gui.menu('表示', 'Overview')
    record = observe(gui, 'initial-invalid-overview')
    assert state()['overview'] and 'Overviewのスライド一覧' in record['tree']
    assert '構文' in gui.text('initial-invalid-diagnostic')
    assert not re.search(r'text (?:\(selected\) )?スライド \d', record['tree'])
    unchanged(before)
    gui.menu('表示', '編集')
    unchanged(before)
    button(gui, '表示')
    button(gui, 'Overview')
    assert state()['overview']
    assert '構文' in gui.text('initial-invalid-toolbar-diagnostic')
    unchanged(before)
    observe(gui, 'initial-invalid-toolbar-overview')


def representative(gui):
    gui.menu('表示', 'Overview')
    record = observe(gui, 'all-elements-overview')
    assert '画像' in record['tree'] or 'Overviewのスライド一覧' in record['tree']
    before_image = Image.open(out / 'all-elements-overview.png').convert('RGB')
    before_colors = Counter(before_image.getdata())
    red_color, red = max(
        ((color, count) for color, count in before_colors.items()
         if color[0] > 150 and color[0] > 2 * color[1] and color[0] > 2 * color[2]),
        key=lambda item: item[1], default=(None, 0),
    )
    assert red > 50, (red_color, red)
    image_state = state()
    Image.new('RGB', (200, 100), (40, 80, 220)).save(out / 'image.png')
    gui.menu('表示', '編集')
    pick(gui, 4, overview=False)
    gui.outline_select('Element 3 · 画像')
    button(gui, '画像を再読み込み')
    pick(gui, 1, overview=False)
    gui.menu('表示', 'Overview')
    unchanged(image_state)
    observe(gui, 'asset-updated-in-grid')
    after_image = Image.open(out / 'asset-updated-in-grid.png').convert('RGB')
    after_colors = Counter(after_image.getdata())
    blue_color, blue = max(
        ((color, count) for color, count in after_colors.items()
         if color[2] > 150 and color[2] > 2 * color[0] and color[2] > 2 * color[1]),
        key=lambda item: item[1], default=(None, 0),
    )
    assert blue > 50 and abs(blue - red) <= red * .05, (red_color, red, blue_color, blue)
    assert after_colors[red_color] == 0, (red_color, after_colors[red_color])
    updated = gui.fixture.read_text().replace('CI のビルドが遅い', 'Updated Overview heading')
    gui.fixture.write_text(updated)
    time.sleep(1.2)
    updated_record = observe(gui, 'external-valid-grid-and-navigator')
    assert updated_record['tree'].count('Updated Overview heading') == 2
    before = state()
    gui.fixture.write_text('presentation {')
    time.sleep(1.2)
    frozen = observe(gui, 'external-invalid-last-good')
    assert state()['sourceHex'] == before['sourceHex'] and not state()['draft']
    assert '一致していません' in gui.text('external-invalid-reason')
    assert 'スライド 1' in frozen['tree']
    gui.menu('表示', '編集')
    assert not state()['overview']
    gui.menu('表示', 'Overview')
    assert state()['overview'] and state()['sourceHex'] == before['sourceHex']
    observe(gui, 'external-invalid-readonly-roundtrip')


def diagnostics(gui):
    before = observe(gui, 'diagnostic-edit')['editor']
    gui.menu('表示', 'Overview')
    overview = observe(gui, 'diagnostic-overview')
    assert overview['editor']['overview'] and overview['editor']['overviewFocused']
    unchanged(before)
    match = re.search(r'^\s*(\d+) button 画像 · Slide 2 · missing-overview.png · .+$',
                      overview['tree'], re.M)
    assert match, overview['tree']
    gui.act('click', '--element-index', match[1], '--no-screenshot')
    selected = observe(gui, 'diagnostic-selected-image')
    assert not state()['overview'] and state()['currentSlide'] == 1
    assert re.search(r'text \(selected\) Element 2 · 画像', selected['tree']), selected['tree']
    assert 'container スライドキャンバス' in selected['tree']
    unchanged(before, ('sourceHex', 'dirty', 'undo', 'redo', 'draft'))
    assert gui.fixture.read_bytes() == gui.original


fixtures = {
    'diagnostics': 'presentation { metadata { title "Diagnostics"; }; slide { heading "First"; }; slide { heading "Image"; image "missing-overview.png"; }; }',
    'guards': 'presentation { metadata { title "Overview guards" }; slide { columns { column width=50 { text "Left"; }; column width=50 { text "Right"; }; }; }; slide { heading "Second"; }; }',
    'empty': 'presentation { metadata { title "Empty"; }; }',
    'invalid': 'presentation { metadata { title "Invalid" }; slide { heading "Never partial"; };',
    'representative': (root / 'examples/build-time.kdl').read_text(),
}
fixture = fixtures.get(args.section, source())
if args.section == 'representative':
    Image.new('RGB', (200, 100), (220, 40, 40)).save(out / 'image.png')
    fixture = fixture.replace('../assets/build-time.png', 'image.png')
    prefix, suffix = fixture.rsplit('}', 1)
    fixture = prefix + 'slide id="blank" {}\n}' + suffix
gui = Gui(out, fixture)
try:
    {'main': main, 'layout': layout, 'guards': guards, 'empty': empty, 'invalid': invalid, 'representative': representative, 'drag': drag_edges, 'resize': resize_drag, 'diagnostics': diagnostics}[args.section](gui)
    (out / 'result.json').write_text(json.dumps(dict(result='PASS', section=args.section, appearance=args.appearance, revision=args.revision,
        macOS=subprocess.check_output(['sw_vers'], text=True)), indent=2))
    print('PASS Overview ' + args.section, flush=True)
finally:
    gui.stop()
