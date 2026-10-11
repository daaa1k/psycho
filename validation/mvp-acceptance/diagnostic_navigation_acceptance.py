"""Native diagnostic navigation and read-only source checks on owned fixtures."""
import argparse
import json
import re
import unicodedata
from pathlib import Path

import numpy as np
from PIL import Image

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/mvp-diagnostic-navigation')
out = parser.parse_args().output.resolve()
assert out.is_relative_to(root / 'target')
out.mkdir(parents=True, exist_ok=True)
records = []


def normalized(text):
    return unicodedata.normalize('NFKC', ''.join(text.split())).replace('・', '·')


def observe(gui, name):
    state, rows = gui.snapshot(name)
    gui.observed_image = gui.out / f'{name}.png'
    gui.observed_scale = state['screenshot'].get('scale', 1)
    records.append(dict(fixture=gui.fixture.name, snapshot=name, ocr=rows,
                        window=state['snapshot']['window']))
    return rows


def readonly_rows(rows):
    return [row for row in rows if '読み取り専用のソース行' in normalized(row['text'])]


def selected_image(rows):
    return [row for row in rows if re.fullmatch(r'Element2·画像[✓く]', normalized(row['text']))]


def header(rows, kind, filename, line=None, column=None):
    pattern = re.compile(re.escape(kind) + r'·' + re.escape(filename) + r':(\d+):(\d+)')
    matches = []
    for row in rows:
        match = pattern.search(normalized(row['text']))
        if match and (line is None or int(match[1]) == line) and (column is None or int(match[2]) == column):
            matches.append((row, int(match[1]), int(match[2])))
    assert len(matches) == 1, dict(expected=(kind, filename, line, column), matches=matches, ocr=rows)
    return matches[0]


def source_matches(row, source_line):
    actual = normalized(row['text'])
    expected = normalized(source_line)
    if '日本語\\q' in source_line:
        return actual.startswith('slide') and '日本語' in actual
    return actual == expected


def source_crop(gui, row, size=None):
    scale = gui.observed_scale
    width, height = size or (round((row['width'] + 12) * scale), round(24 * scale))
    left = round(row['x'] * scale - width / 2)
    top = round(row['y'] * scale - height / 2)
    with Image.open(gui.observed_image) as image:
        return image.convert('RGB').crop((left, top, left + width, top + height))


def dilate(mask):
    padded = np.pad(mask, 1)
    result = np.zeros_like(mask)
    for y in range(3):
        for x in range(3):
            result |= padded[y:y + mask.shape[0], x:x + mask.shape[1]]
    return result


def compare_source(gui, row, reference):
    actual = source_crop(gui, row, reference.size)
    original = np.asarray(reference).min(axis=2)
    observed = np.asarray(actual).min(axis=2)
    assert (original < 100).sum() > 20 and (observed < 100).sum() > 20
    missing_actual = int(((observed < 100) & ~dilate(original < 190)).sum())
    missing_reference = int(((original < 100) & ~dilate(observed < 190)).sum())
    path = gui.observed_image.with_name(gui.observed_image.stem + '-source-strokes.png')
    actual.save(path)
    records.append(dict(fixture=gui.fixture.name, source_stroke_comparison=dict(
        crop=path.name, physical_size=actual.size, tolerance_physical_px=1,
        unmatched_selected=missing_actual, unmatched_diagnostic=missing_reference)))
    assert missing_actual == missing_reference == 0, records[-1]


def selected(gui, rows, line, column, source_line, minimum_count, reference):
    headers = readonly_rows(rows)
    assert len(headers) == 1, rows
    expected = normalized(f'読み取り専用のソース行 · {gui.fixture.name}:{line}:{column}')
    assert normalized(headers[0]['text']) == expected, headers
    matching = [row for row in rows if source_matches(row, source_line)]
    assert len(matching) == minimum_count, dict(expected_source=source_line, rows=rows)
    lower = [row for row in matching if row['y'] > headers[0]['y']]
    assert len(lower) == 1, dict(header=headers, matching=matching)
    compare_source(gui, lower[0], reference)
    records.append(dict(fixture=gui.fixture.name, observed_selection=dict(
        filename=gui.fixture.name, line=line, column=column, source_line=source_line,
        readonly_header=headers[0], selected_source_row=lower[0], source_row_count=len(matching))))
    return lower[0]


def check_invalid(name, source, kind, targets):
    gui = None
    try:
        gui = Gui(out / name, source, name=name)
        rows = observe(gui, 'before-navigation')
        assert not readonly_rows(rows), rows
        for number, (line, column) in enumerate(targets):
            row, actual_line, actual_column = header(rows, kind, gui.fixture.name, line, column)
            source_line = source.splitlines()[actual_line - 1]
            before_count = sum(source_matches(item, source_line) for item in rows)
            assert before_count == 1, dict(source_line=source_line, ocr=rows)
            before_row = next(item for item in rows if source_matches(item, source_line))
            reference = source_crop(gui, before_row)
            reference.save(gui.out / f'diagnostic-source-{number + 1}.png')
            gui.click(row['x'], row['y'])
            rows = observe(gui, f'selected-{number + 1}')
            source_row = selected(gui, rows, actual_line, actual_column, source_line, before_count + 1, reference)
            marker = f'PSYCHO_READONLY_NEGATIVE_{name}_{number}'
            gui.click(source_row['x'], source_row['y'], 2)
            gui.act('type-text', '--text', marker, '--no-screenshot')
            gui.key('Backspace')
            gui.hotkey('CmdOrCtrl+S')
            rows = observe(gui, f'readonly-negative-{number + 1}')
            assert marker not in ''.join(item['text'] for item in rows), rows
            selected(gui, rows, actual_line, actual_column, source_line, before_count + 1, reference)
            assert gui.fixture.read_bytes() == gui.original
            records.append(dict(fixture=gui.fixture.name, input_negative=dict(
                marker=marker, actions=['double-click selected source', 'type marker', 'Backspace', 'Cmd+S'],
                marker_absent=True, source_bytes_unchanged=True)))
        print(f'PASS {name}: diagnostic selection, source line and read-only input', flush=True)
    finally:
        try:
            if gui is not None:
                gui.stop()
        finally:
            (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')


def check_asset():
    gui = None
    source = '''presentation {
 metadata { title "Asset diagnostic navigation" }
 slide { heading "First slide"; }
 slide { heading "Missing image slide"; image "diagnostic-missing-image.png"; }
}
'''
    try:
        gui = Gui(out / 'asset', source, name='asset')
        rows = observe(gui, 'asset-before-navigation')
        assert any(normalized(row['text']) == 'Firstslide' and row['x'] > 160 for row in rows), rows
        assert not selected_image(rows), rows
        headers = [row for row in rows if normalized(row['text']).startswith(
            '画像·Slide2·diagnostic-missing-image.png·')]
        assert len(headers) == 1, rows
        gui.click(headers[0]['x'], headers[0]['y'])
        rows = observe(gui, 'asset-selected-image')
        assert any(normalized(row['text']) == 'Missingimageslide' and row['x'] > 160 for row in rows), rows
        assert len(selected_image(rows)) == 1, rows
        assert any(normalized(row['text']) == '画像パス' for row in rows), rows
        input_state = gui.state()
        assert input_state['text'] == 'diagnostic-missing-image.png', input_state
        assert input_state['undo'] == 0 and input_state['redo'] == 0, input_state
        gui.hotkey('CmdOrCtrl+Z')
        rows = observe(gui, 'asset-after-undo')
        assert len(selected_image(rows)) == 1, rows
        assert any(normalized(row['text']) == 'Missingimageslide' and row['x'] > 160 for row in rows), rows
        after = gui.state()
        assert after['text'] == input_state['text'] and after['undo'] == 0 and after['redo'] == 0, after
        gui.hotkey('CmdOrCtrl+S')
        assert gui.fixture.read_bytes() == gui.original
        records.append(dict(fixture=gui.fixture.name, observed_asset_selection=dict(
            slide=2, element='image', path=after['text'], before_undo=input_state, after_undo=after,
            source_bytes_unchanged=True)))
        print('PASS asset: footer diagnostic selects slide 2 image without history', flush=True)
    finally:
        try:
            if gui is not None:
                gui.stop()
        finally:
            (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')


check_invalid('syntax', 'presentation {\n metadata { title "x" }\n slide { text "日本語\\q"; }\n}\n',
              'KDL構文', [(3, None)])
check_invalid('schema', 'presentation {\n metadata { title "x" }\n slide id="diag" {\n  text 42\n  heading 42\n }\n}\n',
              'Schema', [(4, 3), (5, 3)])
check_asset()
