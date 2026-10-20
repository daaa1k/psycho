import argparse
import json
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Present workflow" }; slide { heading "First slide"; }; slide { heading "Second slide"; }; }\n')
records = []

def observe(name, expected, fullscreen):
    state, rows = gui.snapshot(name)
    text = ''.join(row['text'] for row in rows)
    assert expected in text, text
    assert (state['snapshot']['window']['width'] > 1280) == fullscreen, state['snapshot']['window']
    assert gui.fixture.read_bytes() == gui.original
    records.append(dict(name=name, tree=state['snapshot']['treeText'], window=state['snapshot']['window'],
                        scale=state['screenshot'].get('scale'), input=gui.state()))

try:
    gui.select_slide(2)
    gui.edit('Second slide', 48)
    gui.replace('Unsaved second')
    gui.button('プレゼンテーション')
    time.sleep(.8)
    observe('present-current-unsaved', 'Unsaved second', True)
    gui.key('Escape')
    time.sleep(.8)
    observe('return-current-selection', 'Unsaved second', False)
    gui.button('発表メニュー')
    gui.button('最初からプレゼンテーション')
    time.sleep(.8)
    observe('present-first', 'First slide', True)
    gui.key('Right')
    observe('present-next-unsaved', 'Unsaved second', True)
    gui.key('Escape')
    time.sleep(.8)
    observe('return-final-selection', 'Unsaved second', False)
    assert 'selected) スライド 2 Unsaved second' in records[-1]['tree'], records[-1]
    gui.key('Tab')
    gui.press('保存')
    assert b'heading "Unsaved second"' in gui.fixture.read_bytes()
    print('PASS current/first fullscreen, committed unsaved draft, navigation, selection/focus return and no autosave')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
