"""Public divider drag acceptance. Run only when no other GUI driver is active.

Coordinates come from the two canvas glyph origins and main.rs's 24*scale
column gutter. No hard-coded screen position or test-only mutation is used.
"""
import argparse
import csv
import json
import re
import shutil
import subprocess
import time
from pathlib import Path

from gui_session import Gui, root

SOURCE = '''presentation {
 metadata { title "Column divider acceptance" }
 slide {
  heading "Divider acceptance"
  columns { column width=50 { text "LEFT"; }; column width=50 { text "RIGHT"; }; }
 }
}
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to(root / 'target'), 'Output must be under target/'
    assert not output.exists(), 'Use a fresh output directory'
    gui = Gui(output, SOURCE, 'column-width-drag')
    records = []
    result = dict(status='RUNNING', records=records,
                  head=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
                  owned_pid=gui.app.pid,
                  coordinates='Latest canvas glyph origins; gutter 24*scale from main.rs')

    def origin(text):
        rows = list(csv.reader(gui.trace.open()))
        row = next(row for row in reversed(rows)
                   if len(row) == 12 and row[4] == '0'
                   and bytes.fromhex(row[3]).decode() == text and float(row[2]) == 28)
        return float(row[6]), float(row[7]), float(row[8])

    def capture(name):
        state = gui.act('get-app-state')
        shutil.copy(state['screenshot']['path'], output / f'{name}.png')
        left = origin('LEFT')
        right = origin('RIGHT')
        observation = dict(name=name, time=time.monotonic(), left=left, right=right,
                           source_on_disk=gui.fixture.read_text())
        records.append(observation)
        return observation

    def widths(data):
        return list(map(int, re.findall(rb'column width=(\d+)', data)))

    try:
        helper = output / 'slow-drag'
        subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('slow_drag.swift')),
                        '-o', str(helper)], check=True, capture_output=True)
        baseline = gui.save()
        assert baseline == gui.original and widths(baseline) == [50, 50]
        for name, target in [('continuous', 65), ('lower-limit', -20), ('upper-limit', 120)]:
            gui.snapshot(name + '-before')
            lx, ly, scale = origin('LEFT')
            rx, _, _ = origin('RIGHT')
            usable = (rx - lx - 24 * scale) / .5
            assert usable > 100, ('Unexpected canvas geometry', lx, rx, scale)
            divider = lx + usable * .5 + 12 * scale
            y = ly + 27
            endpoint = divider + usable * (target - 50) / 100
            gui.act('get-app-state', '--restore-window')
            process = subprocess.Popen([str(helper), str(gui.app.pid), str(divider), str(y),
                                        str(endpoint), str(y), '7'],
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                time.sleep(.65)
                mid = capture(name + '-moving')
                time.sleep(1.4)
                held = capture(name + '-held')
                assert process.poll() is None, 'Mouse must still be held during observation'
                held_width = round((held['right'][0] - held['left'][0] - 24 * scale) / usable * 100)
                expected = max(1, min(99, target))
                assert abs(held_width - expected) <= 1, (name, held_width, expected)
                if name == 'continuous':
                    moving_width = (mid['right'][0] - mid['left'][0] - 24 * scale) / usable * 100
                    assert 50 < moving_width < held_width, ('No intermediate width', moving_width, held_width)
                assert gui.fixture.read_bytes() == baseline, 'Held preview must not write disk'
                # A keyboard Save preserves the held mouse; clicking Save would
                # release it and invalidate the before-drop observation.
                gui.hotkey('CmdOrCtrl+S')
                assert process.poll() is None
                assert gui.fixture.read_bytes() == baseline, 'Save while held must retain uncommitted source'
                capture(name + '-held-after-save')
            finally:
                stdout, stderr = process.communicate(timeout=20)
                assert process.returncode == 0, (stdout, stderr)
            gui.snapshot(name + '-released')
            after = gui.save()
            assert widths(after) == [expected, 100 - expected], (name, after)
            assert after != baseline
            (output / f'{name}-saved.kdl').write_bytes(after)
            gui.press('Undo')
            assert gui.save() == baseline, 'One Undo must restore every original byte'
            gui.snapshot(name + '-undo')
            gui.press('Redo')
            assert gui.save() == after, 'One Redo must restore every changed byte'
            gui.snapshot(name + '-redo')
            gui.stop()
            gui.start()
            assert gui.fixture.read_bytes() == after
            gui.snapshot()
            px, py = gui.glyph_position('LEFT' if expected >= 50 else 'RIGHT', 28)
            gui.click(px, py)
            gui.press('2列の内容へ戻る')
            assert f'左列{expected}%/右列{100 - expected}%' in gui.text(name + '-reloaded'), 'Reloaded inspector widths'
            assert gui.save() == after, 'Reload and no-op Save must preserve bytes'
            gui.stop()
            gui.fixture.write_bytes(baseline)
            gui.start()
            records.append(dict(name=name, expected=expected, saved_source=after.decode(),
                                exact_undo=True, exact_redo=True, persisted_reload=True))
            print('PASS', name, 'divider preview, clamp, one Undo/Redo, Save/reload', flush=True)
        result['status'] = 'PASS'
    except Exception as error:
        result['status'] = 'FAIL'
        result['error'] = repr(error)
        try:
            gui.snapshot('column-width-failure')
        except Exception as capture_error:
            result['capture_error'] = repr(capture_error)
        raise
    finally:
        (output / 'column-width-drag-observations.json').write_text(
            json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        gui.stop()


if __name__ == '__main__':
    main()
