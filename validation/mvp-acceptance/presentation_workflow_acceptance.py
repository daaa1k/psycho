import argparse
import json
import subprocess
import time
from pathlib import Path

from PIL import Image

from gui_session import Gui as BaseGui, root


class Gui(BaseGui):
    def start(self):
        super().start()
        self.click(900, 750)


SOURCE = ('presentation { metadata { title "Presentation acceptance" }; '
          'slide id="one" { heading "First slide"; }; '
          'slide id="two" { heading "Second slide"; image "marker.png"; }; '
          'slide id="three" { heading "Third slide"; }; }\n')
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/issue29-presentation')
parser.add_argument('--section', action='append', choices=['snapshot', 'preflight', 'keys', 'positions', 'external'])
parser.add_argument('--cycles', type=int, default=10)
args = parser.parse_args()
assert args.cycles > 0
records = []


def observe(gui, name, expected, fullscreen=False, presenting=True):
    state, rows = gui.snapshot(name)
    text = ''.join(''.join(row['text'].split()) for row in rows)
    assert ''.join(expected.split()) in text, (name, expected, text)
    assert state['snapshot']['window']['width'] == (1920 if fullscreen else 1280), state
    if fullscreen and presenting:
        assert not any(label in text for label in ['Inspector', '最初から発表', '現在から発表']), text
    if expected in {'First slide', 'Second slide', 'Third slide', 'Unsaved first', 'Local unsaved',
                    'Third updated', 'New third', 'Only remaining'}:
        assert any(''.join(row['text'].split()) == ''.join(expected.split())
                   and (fullscreen or 160 < row['x'] < 995) for row in rows), (name, rows)
    record = dict(case=name, expected=expected, fullscreen=fullscreen,
                  input=gui.state(), window=state['snapshot']['window'])
    records.append(record)
    (args.output / 'observations.json').write_text(
        json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    print('PASS', name, flush=True)
    return state, rows


def marker(gui, color):
    Image.new('RGB', (80, 40), color).save(gui.out / 'marker.png')


def pixels(gui, name, color):
    gui.snapshot(name)
    image = Image.open(gui.out / (name + '.png')).convert('RGB')
    channel = 0 if color == (255, 0, 0) else 2
    return sum(pixel[channel] > 200 and all(pixel[other] < 100 for other in range(3) if other != channel)
               for pixel in image.getdata())


def start(gui, label='現在から発表'):
    gui.press(label)
    time.sleep(.8)


def exit_presentation(gui):
    gui.key('Escape')
    time.sleep(.8)
    assert not gui.state()['focused'], 'Exit must focus the canvas, not a text field'


def check_snapshot():
    gui = Gui(args.output / 'snapshot', SOURCE)
    try:
        marker(gui, (255, 0, 0))
        gui.edit('First slide', 48)
        gui.replace('Unsaved first')
        start(gui, '最初から発表')
        observe(gui, 'unsaved-without-save', 'Unsaved first', True)
        assert gui.fixture.read_bytes() == gui.original
        gui.key('Right')
        observe(gui, 'red-image-start', 'Second slide', True)
        red = pixels(gui, 'red-image', (255, 0, 0))
        assert red > 10000, red
        frame = Image.open(gui.out / 'red-image.png').convert('RGB')
        assert frame.getpixel((960, 30)) == frame.getpixel((960, 1170)) == (0, 0, 0)
        assert frame.getpixel((20, 600)) == (255, 255, 255)
        marker(gui, (0, 0, 255))
        assert pixels(gui, 'updated-image-stays-red', (255, 0, 0)) == red
        (gui.out / 'marker.png').unlink()
        gui.key('Right')
        gui.key('Left')
        assert pixels(gui, 'deleted-image-stays-red', (255, 0, 0)) == red
        assert pixels(gui, 'deleted-image-no-blue', (0, 0, 255)) == 0
        exit_presentation(gui)
        observe(gui, 'exit-last-slide-and-missing-image', '画像ファイルがありません')
        assert gui.fixture.read_bytes() == gui.original
        gui.key('Space')
        observe(gui, 'exit-space-does-not-restart', 'Second slide')
        gui.select_slide(1)
        start(gui)
        observe(gui, 'other-slide-image-blocks-start', '発表を開始できません')
        marker(gui, (0, 0, 255))
        start(gui)
        gui.key('Right')
        assert pixels(gui, 'restart-uses-blue', (0, 0, 255)) > 10000
        exit_presentation(gui)
        gui.press('保存')
        assert b'Unsaved first' in gui.fixture.read_bytes()
    finally:
        gui.stop()


def check_preflight():
    gui = Gui(args.output / 'prestart-delete', SOURCE)
    try:
        marker(gui, (255, 0, 0))
        gui.stop()
        gui.start()
        (gui.out / 'marker.png').unlink()
        start(gui, '最初から発表')
        observe(gui, 'deleted-immediately-before-start', '画像ファイルがありません')
        assert gui.fixture.read_bytes() == gui.original
    finally:
        gui.stop()

    for case, source, expected in [
        ('empty', 'presentation { metadata { title "Empty" }; }', 'Slide がない'),
        ('invalid-document', 'presentation { metadata { title "Invalid" }; slide { unknown; }; }', 'Schema'),
        ('noncurrent-layout', SOURCE.replace('image "marker.png";', 'code "' + 'W' * 200 + '";'), 'はみ出します'),
    ]:
        gui = Gui(args.output / case, source)
        try:
            start(gui)
            observe(gui, case + '-stays-in-editor', expected)
            assert gui.fixture.read_bytes() == gui.original
        finally:
            gui.stop()

    gui = Gui(args.output / 'invalid-input', SOURCE.replace('image "marker.png";', ''))
    try:
        gui.press('Presentation title')
        _, rows = gui.snapshot()
        title = next(row for row in rows if row['text'] == 'Presentation acceptance' and row['x'] > 995)
        gui.click(title['x'], title['y'])
        gui.replace('')
        start(gui)
        observe(gui, 'invalid-input-stays-in-editor', 'タイトルを入力してください')
        assert gui.fixture.read_bytes() == gui.original
    finally:
        gui.stop()


def check_keys():
    gui = Gui(args.output / 'keys', SOURCE.replace('image "marker.png";', ''))
    try:
        headings = ['First slide', 'Second slide', 'Third slide']
        for cycle in range(args.cycles):
            gui.select_slide(2)
            start(gui, '最初から発表' if cycle % 2 == 0 else '現在から発表')
            index = 0 if cycle % 2 == 0 else 1
            observe(gui, f'cycle-{cycle + 1}-start', headings[index], True)
            if cycle == 0:
                subprocess.run(['osascript', '-e', 'tell application "Finder" to activate'], check=True)
                time.sleep(.5)
                gui.act('get-app-state', '--restore-window')
                observe(gui, 'other-app-return', headings[index], True)
            for key in (['Right', 'Down', 'Space', 'PageDown'] if cycle == 0 else ['Right']):
                if cycle == 0 and index == 2:
                    gui.key('Left')
                    index -= 1
                gui.key(key)
                index = min(index + 1, 2)
                observe(gui, f'cycle-{cycle + 1}-{key}', headings[index], True)
            if cycle == 0:
                gui.key('Right')
                observe(gui, 'last-slide-does-not-wrap', headings[2], True)
            for key in (['Left', 'Up', 'PageUp'] if cycle == 0 else ['Left']):
                if cycle == 0 and index == 0:
                    gui.key('Right')
                    index += 1
                gui.key(key)
                index = max(index - 1, 0)
                observe(gui, f'cycle-{cycle + 1}-{key}', headings[index], True)
            if cycle == 0:
                gui.key('Left')
                observe(gui, 'first-slide-does-not-wrap', headings[0], True)
            gui.click(600, 350)
            gui.act('scroll', '--x', '600', '--y', '350', '--direction', 'down', '--no-screenshot')
            observe(gui, f'cycle-{cycle + 1}-pointer-keeps-position', headings[index], True)
            exit_presentation(gui)
            observe(gui, f'cycle-{cycle + 1}-return', headings[index])
        gui.select_slide(1)
        gui.click(60, 13)
        time.sleep(1)
        observe(gui, 'editor-already-fullscreen', 'First slide', True, presenting=False)
        start(gui)
        observe(gui, 'start-keeps-fullscreen', 'First slide', True)
        exit_presentation(gui)
        observe(gui, 'fullscreen-editor-return', 'First slide', True, presenting=False)
    finally:
        gui.stop()


def check_positions():
    for case, external, expected in [
        ('same-id', 'slide id="three" { heading "Third updated"; }; slide id="one" { heading "First updated"; };', 'Third updated'),
        ('same-index', 'slide { heading "New first"; }; slide { heading "New second"; }; slide { heading "New third"; };', 'New third'),
        ('last-index', 'slide { heading "Only remaining"; };', 'Only remaining'),
        ('no-slides', '', '+ Slide'),
    ]:
        gui = Gui(args.output / case, SOURCE.replace('image "marker.png";', ''))
        try:
            gui.select_slide(3)
            start(gui)
            updated = 'presentation { metadata { title "External update" }; ' + external + ' }\n'
            gui.fixture.write_text(updated)
            time.sleep(.8)
            observe(gui, case + '-snapshot', 'Third slide', True)
            exit_presentation(gui)
            observe(gui, case + '-return', expected)
            assert gui.fixture.read_text() == updated
            if case != 'no-slides':
                start(gui)
                observe(gui, case + '-restart', expected, True)
                exit_presentation(gui)
            else:
                start(gui)
                observe(gui, 'no-slides-restart-blocked', 'Slide がない')
        finally:
            gui.stop()


def check_external():
    for case in ['conflict', 'deleted', 'invalid']:
        gui = Gui(args.output / case, SOURCE.replace('image "marker.png";', ''))
        try:
            if case == 'conflict':
                gui.edit('First slide', 48)
                gui.replace('Local unsaved')
            start(gui, '最初から発表')
            if case == 'deleted':
                gui.fixture.unlink()
            elif case == 'invalid':
                gui.fixture.write_text('presentation {')
            else:
                gui.fixture.write_text(SOURCE.replace('image "marker.png";', '').replace('First slide', 'External first'))
            observe(gui, case + '-frozen', 'Local unsaved' if case == 'conflict' else 'First slide', True)
            exit_presentation(gui)
            observe(gui, case + '-exit-blocked', {'conflict': '競合', 'deleted': '削除', 'invalid': '無効'}[case])
            start(gui)
            observe(gui, case + '-restart-blocked', {'conflict': '競合', 'deleted': '削除', 'invalid': '無効'}[case])
            if case == 'conflict':
                archive = gui.out / 'archive'
                archive.mkdir()
                original_external = gui.fixture.read_bytes()
                gui.press('別名保存')
                state = gui.act('get-app-state')
                row = next(line for line in state['snapshot']['treeText'].splitlines() if line.strip().endswith('button 退避先を選ぶ'))
                gui.act('click', '--element-index', row.strip().split()[0], '--no-screenshot')
                gui.hotkey('CmdOrCtrl+Shift+G')
                gui.hotkey('CmdOrCtrl+A')
                gui.act('type-text', '--text', str(archive.resolve()), '--no-screenshot')
                gui.key('Return')
                state = gui.act('get-app-state')
                assert 'Where:, Value: archive' in state['snapshot']['treeText'], state
                row = next(line for line in state['snapshot']['treeText'].splitlines() if line.strip().endswith('button Save'))
                gui.act('click', '--element-index', row.strip().split()[0], '--no-screenshot')
                observe(gui, 'conflict-retreat-preserves-content', 'Local unsaved')
                saved = archive / 'presentation.kdl'
                assert b'Local unsaved' in saved.read_bytes()
                assert gui.fixture.read_bytes() == original_external
                start(gui)
                observe(gui, 'retreat-can-present', 'Local unsaved', True)
                exit_presentation(gui)
            else:
                gui.fixture.write_bytes(gui.original)
                gui.press('外部変更を読み込む')
                observe(gui, case + '-recovered', 'First slide')
        finally:
            gui.stop()


checks = {
    'snapshot': check_snapshot,
    'preflight': check_preflight,
    'keys': check_keys,
    'positions': check_positions,
    'external': check_external,
}
for section in args.section or checks:
    checks[section]()
print('PASS presentation sections ' + ', '.join(args.section or checks), flush=True)
