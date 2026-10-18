import argparse
import json
import os
import subprocess
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
out = parser.parse_args().output
out.mkdir(parents=True, exist_ok=True)
helper = out.resolve() / 'resize-window'
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('resize_window.swift')), '-o', str(helper)], check=True)
previous = os.environ.get('PSYCHO_ACCEPTANCE_APPEARANCE')
records = []
try:
    for theme in ('light', 'dark'):
        os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = theme
        gui = Gui(out / theme,
                  'presentation { metadata { title "Document layout" }; slide { heading "Retained heading"; image "missing.png"; }; }\n')
        try:
            gui.fixture.write_text('presentation { slide { heading } }\n')
            time.sleep(1.2)
            for width, height in ((1024, 732), (1280, 892), (1440, 992)):
                subprocess.run([str(helper), str(gui.app.pid), str(width), str(height)], check=True)
                time.sleep(.4)
                state, rows = gui.snapshot(f'banner-{width}')
                assert state['snapshot']['window']['width'] == width
                tree = state['snapshot']['treeText']
                assert all(label in tree for label in ('button 再読み込み', 'button 別名保存', 'button 発表メニュー', '外部ファイルが無効'))
                gui.diagnostics()
                state, rows = gui.snapshot(f'diagnostics-{width}')
                assert 'container 診断' in state['snapshot']['treeText']
                assert 'button Schema' in state['snapshot']['treeText']
                records.append(dict(theme=theme, window=state['snapshot']['window'], scale=state['screenshot'].get('scale')))
                gui.key('Escape')
        finally:
            gui.stop()
    print('PASS document banner and diagnostics in light/dark at 1024x700, 1280x860 and 1440x960 editor sizes')
finally:
    if previous is None:
        os.environ.pop('PSYCHO_ACCEPTANCE_APPEARANCE', None)
    else:
        os.environ['PSYCHO_ACCEPTANCE_APPEARANCE'] = previous
    (out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
