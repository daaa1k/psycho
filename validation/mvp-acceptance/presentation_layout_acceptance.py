import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/issue29-presentation-layout')
args = parser.parse_args()
directory = args.output.resolve()
source = Path(__file__).with_name('layout.kdl').read_text().replace(
    '../../assets/build-time.png', os.path.relpath(root / 'assets/build-time.png', directory))
gui = Gui(directory, source)
try:
    gui.click(900, 750)
    for mode, width, height in [('small', 1280, 892), ('large', 1440, 992)]:
        subprocess.run(['xcrun', 'swift', str(Path(__file__).with_name('resize_window.swift')),
                        str(gui.app.pid), str(width), str(height)], check=True, capture_output=True)
        time.sleep(.5)
        for slide in [1, 2]:
            gui.select_slide(slide)
            gui.snapshot(f'layout-{mode}-{slide}')
    gui.press('最初から発表')
    time.sleep(1)
    gui.snapshot('layout-fullscreen-1')
    gui.key('Right')
    gui.snapshot('layout-fullscreen-2')
    gui.key('Escape')
    time.sleep(1)
    shutil.copyfile(gui.trace, gui.out / 'glyph-origins.csv')
    assert gui.fixture.read_bytes() == gui.original
    subprocess.run([sys.executable, str(Path(__file__).with_name('measure_layout.py')),
                    '--evidence', str(gui.out)], check=True)
    print('PASS identical Japanese glyphs, wrapping and base placement in two editor sizes and fullscreen', flush=True)
finally:
    gui.stop()
