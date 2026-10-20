import argparse
import json
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
large = 'overflow();\n' * 30
source = ('presentation { metadata { title "Draft guard" }; slide { heading "First slide"; }; '
          'slide { heading "Layout problem"; code ' + json.dumps(large) + '; }; }\n')
gui = Gui(parser.parse_args().output, source)
records = []
try:
    gui.button('Presentation title')
    _, rows = gui.snapshot('title-before-invalid-draft')
    field = next(row for row in rows if row['text'] == 'Draft guard' and row['x'] > 1000)
    gui.click(field['x'], field['y'])
    gui.replace('')
    gui.diagnostics()
    gui.key('Tab')
    gui.key('Return')
    state, _ = gui.snapshot('invalid-draft-retained-at-layout-diagnostic')
    tree = state['snapshot']['treeText']
    assert gui.state()['text'] == '', gui.state()
    assert '(selected) スライド 1 First slide' in tree, tree
    assert '(selected) スライド 2' not in tree, tree
    assert 'container 診断' in tree, tree
    assert gui.fixture.read_bytes() == gui.original
    records.append(dict(tree=tree, input=gui.state(), source=gui.fixture.read_text()))
    gui.key('Escape')
    assert gui.state()['focused'], gui.state()
    assert gui.state()['text'] == ''
    print('PASS invalid draft diagnostic navigation preserves slide, target, popup, bytes and cancel focus')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
