import argparse
import csv
import difflib
import json
import re
import subprocess
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--ime', action='store_true')
parser.add_argument('--direct-and-reload', action='store_true')
parser.add_argument('--direct-start', type=int, default=3)
args = parser.parse_args()
large = 'overflow();\n' * 30
source = ('presentation { metadata { title "Inspector acceptance" }; '
          'slide { heading "Heading"; text "Body"; bullets { item "First"; item "Second"; }; '
          'code "println!();" language="unlisted-language"; image "image.png" { caption "Caption"; }; '
          'columns { column width=50 { text "Left"; }; column width=50 { text "Right"; }; }; '
          'code ' + json.dumps(large) + '; ' + ''.join('text "Hidden ' + str(i) + '"; ' for i in range(8, 21)) +
          '}; slide { heading "IME heading"; text "IME body"; }; slide { bullets { item "Direct bullet"; }; }; slide { code "Direct code"; }; slide { image "image.png" { caption "Direct caption"; }; }; }\n')
labels = ['Element 1 · 見出し', 'Element 2 · 本文', 'Element 3 · 箇条書き',
          'Element 4 · コード', 'Element 5 · 画像', 'Element 6 · 2列',
          '2列 6 · 左列 · 本文 1', '2列 6 · 右列 · 本文 1', 'Element 7 · コード'] + ['Element ' + str(i) + ' · 本文' for i in range(8, 21)]
gui = Gui(args.output, source)
records = []
def scroll(y, direction):
    gui.inspector_scroll(y, direction)


def tree():
    return gui.act('get-app-state')['snapshot']['treeText']


def ax(label, settings=True):
    for _ in range(18):
        value = tree()
        match = re.search(r'^\s*(\d+) (?:button|text|text field) (?:\([^)]*\) )?' + re.escape(label) + r'(?: ✓)?(?:, .*)?$', value, re.M)
        if match:
            if 'text field' in match.group(0):
                gui.focus_input(label)
            else:
                gui.act('click', '--element-index', match[1], '--no-screenshot')
            return
        if settings:
            scroll(580, 'down')
        else:
            visible = [i for i, name in enumerate(labels) if name in value]
            direction = 'up' if visible and labels.index(label) < min(visible) else 'down'
            scroll(265, direction)
    raise AssertionError((label, value))


def select(label):
    ax(label, False)
    value = tree()
    assert re.search(r'text \(selected\) ' + re.escape(label), value), value
    assert '次の項目' not in value and '前の項目' not in value, value


def record(name):
    state, _ = gui.snapshot(name)
    records.append(dict(name=name, tree=state['snapshot']['treeText'], input=gui.state(), kdl=gui.fixture.read_text()))
    print('PASS', name, flush=True)


def field(label, value):
    ax(label)
    gui.replace(value)
    ax('適用')


def glyphs(text, font):
    rows = list(csv.reader(gui.trace.open()))
    groups = []
    current = []
    for row in rows:
        if len(row) != 12 or bytes.fromhex(row[3]).decode() != text or float(row[2]) != font or float(row[8]) < .3:
            continue
        if row[4] == '0':
            if current: groups.append(current)
            current = []
        current.append(row)
    if current: groups.append(current)
    assert groups, ('Missing canvas glyph trace', text, font)
    selected = groups[-1]
    return [(row[5], (float(row[6]) - float(row[9])) / float(row[8]), (float(row[7]) - float(row[10])) / float(row[8])) for row in selected]


def direct(text, font):
    gui.snapshot()
    before = glyphs(text, font)
    gui.edit(text, font)
    gui.snapshot()
    after = glyphs(text, font)
    assert len(after) == len(before) and len(after) >= len(text), (before, after)
    for a, b in zip(before, after):
        assert a[0] == b[0] and abs(a[1] - b[1]) < .001 and abs(a[2] - b[2]) < .001, (a, b)
    assert gui.state()['focused'] and gui.state()['text'] == text
    records.append(dict(name='glyph-parity-' + text, before=before, after=after))


try:
    (gui.out / 'image.png').write_bytes((root / 'assets/build-time.png').read_bytes())
    (gui.out / 'second.png').write_bytes((root / 'assets/build-time.png').read_bytes())
    if not args.direct_and_reload:
        ax('Presentation title')
        assert 'Presentation のタイトル' in tree(), tree()
        assert gui.state()['text'] == 'Inspector acceptance'
        record('background-title')
        select('Element 20 · 本文')
        assert gui.state()['text'] == 'Hidden 20'
        assert 'text field (settable) 本文' in tree(), tree()
        record('last-outline-settings-visible')
        field('本文', 'Last repaired')
        saved = gui.save()
        assert saved.decode() == source.replace('Hidden 20', 'Last repaired')
        gui.press('Undo'); assert gui.save() == gui.original
        gui.press('Redo'); assert gui.save() == saved
        record('offscreen-history')
        select('2列 6 · 左列 · 本文 1'); field('本文', 'Left repaired')
        select('2列 6 · 右列 · 本文 1'); field('本文', 'Right repaired')
        saved = gui.save()
        assert b'Left repaired' in saved and b'Right repaired' in saved
        record('nested-both-columns')
        select('Element 4 · コード')
        assert 'unlisted-language' in tree() or 'unlisted-language' in gui.text(), tree()
        assert gui.save() == saved, 'Selecting an unknown language must preserve KDL'
        ax('言語をrustに変更')
        assert b'language="rust"' in gui.save()
        ax('コード言語'); field('コード言語、任意の文字列', 'custom-language')
        assert b'language="custom-language"' in gui.save()
        record('preset-and-arbitrary-language')
        select('Element 6 · 2列'); ax('左列幅を数値入力')
        ax('左列幅、1〜99の整数'); gui.replace('0')
        record('invalid-width-reason')
        before = gui.fixture.read_bytes()
        ax('Presentation title')
        assert gui.state()['text'] == '0', gui.state()
        select_attempt = tree()
        assert '左列幅、1〜99の整数' in select_attempt, select_attempt
        gui.menu('表示', 'Inspectorを隠す')
        assert 'スライドの構造一覧' in tree(), tree()
        assert gui.fixture.read_bytes() == before
        ax('入力を取り消す'); assert gui.state()['text'] == '50'
        ax('左列幅、1〜99の整数'); gui.replace('60'); ax('幅を適用')
        assert b'column width=60' in gui.save() and b'column width=40' in gui.fixture.read_bytes()
        record('width-validation-cancel-apply')
        select('Element 5 · 画像'); ax('画像パス'); field('画像パス、文書からの相対パス', 'second.png')
        ax('Caption'); field('画像のCaption', 'New caption')
        assert b'image "second.png"' in gui.save() and b'New caption' in gui.fixture.read_bytes()
        record('image-reference-caption')
        select('Element 3 · 箇条書き'); ax('項目 1'); field('箇条書きの項目', 'Updated item')
        ax('項目を下へ'); ax('項目を追加'); gui.replace('Added item'); ax('適用')
        assert b'Added item' in gui.save()
        ax('項目を削除'); assert b'Added item' not in gui.save()
        record('bullet-item-operations')
        select('Element 2 · 本文')
        ax('本文'); gui.replace('Pane retained')
        gui.menu('表示', 'Inspectorを隠す'); gui.menu('表示', 'Inspectorを表示')
        assert gui.state()['text'] == 'Pane retained'
        assert 'Element 2 · 本文' in tree()
        pane = gui.save()
        gui.menu('表示', 'Inspectorを広くする'); gui.menu('表示', 'Inspectorを狭くする')
        assert gui.save() == pane and gui.state()['text'] == 'Pane retained'
        record('pane-selection-value-history')
        ax('Element ↓'); moved = gui.save(); assert moved != pane
        gui.press('Undo'); assert gui.save() == pane
        ax('Element を削除'); deleted = gui.save(); assert deleted != pane
        gui.press('Undo'); assert gui.save() == pane
        record('move-delete-history')
    gui.select_slide(2)
    direct('IME body', 28)
    gui.key('Escape')
    direct('IME heading', 48)
    assert gui.state()['focused'] and gui.state()['text'] == 'IME heading'
    assert '(selected) Element 1 · 見出し' in tree()
    record('canvas-outline-selection')
    ax('全文をInspectorで編集')
    assert gui.state()['focused'] and gui.state()['text'] == 'IME heading'
    if args.ime:
        gui.edit('IME heading', 48); gui.hotkey('CmdOrCtrl+A')
        for key in 'nihongonohenkann': gui.key(key)
        gui.key('Space')
        before = gui.state()
        assert before['composing'] and before['marked'][0] >= 0, before
        record('native-ime-canvas')
        ax('全文をInspectorで編集')
        after = gui.state()
        for key in ('text', 'selected', 'marked', 'undo', 'redo', 'composing', 'focused'):
            assert after[key] == before[key], (key, before, after)
        record('native-ime-inspector')
        gui.edit('IME heading', 48)
        after = gui.state()
        for key in ('text', 'selected', 'marked', 'undo', 'redo', 'composing', 'focused'):
            assert after[key] == before[key], (key, before, after)
        record('native-ime-canvas-return')

        for _ in range(5):
            if not gui.state()['composing']: break
            gui.key('Return')
        gui.key('Escape')
        assert not gui.state()['composing']
        saved = gui.save(); assert before['text'].encode() in saved
    else:
        field('見出し', 'Edited heading')
    gui.key('Escape')
    for slide, text, font in [(3, 'Direct bullet', 28), (4, 'Direct code', 22), (5, 'Direct caption', 20)]:
        if slide < args.direct_start: continue
        gui.select_slide(slide)
        direct(text, font)
        ax('全文をInspectorで編集')
        assert gui.state()['text'] == text and gui.state()['focused']
        record('direct-full-editor-' + str(slide))
        gui.key('Escape')
    final = gui.save()
    (gui.out / 'saved.diff').write_text(''.join(difflib.unified_diff(source.splitlines(True), final.decode().splitlines(True))))
    gui.stop(); gui.start(); gui.select_slide(2)
    select('Element 1 · 見出し')
    expected = before['text'] if args.ime else 'Edited heading'
    assert gui.state()['text'] == expected
    assert gui.fixture.read_bytes() == final
    record('saved-reload')
    gui.click(920, 160)
    assert gui.state()['text'] == 'Inspector acceptance' and 'text field (settable) Presentation のタイトル' in tree()
    assert 'キャンバス上で編集中' not in tree()
    record('background-clears-canvas-editor')
    select('Element 2 · 本文'); ax('本文'); gui.replace('Pending conflict')
    gui.fixture.write_text(gui.fixture.read_text().replace('Inspector acceptance', 'External title'))
    import time
    time.sleep(1.5)
    blocked = tree()
    assert '外部ファイルの問題' in gui.text(), blocked
    assert gui.state()['text'] == 'Pending conflict'
    record('external-conflict-stops-editing')
except Exception:
    gui.snapshot('failure')
    raise
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
