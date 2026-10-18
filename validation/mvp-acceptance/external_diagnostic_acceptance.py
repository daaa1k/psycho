import argparse
import json
import re
import time
from pathlib import Path
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Retained model" }; slide { heading "Retained heading"; }; }\n')
records = []
try:
    gui.edit('Retained heading', 48)
    gui.replace('Preserved preview')
    invalid = 'presentation {\n metadata { title "External invalid" }\n slide { heading "bad\\q"; }\n}\n'
    gui.fixture.write_text(invalid)
    time.sleep(1.2)
    gui.diagnostics()
    for _ in range(25):
        tree = gui.act('get-app-state')['snapshot']['treeText']
        if re.search(r'The focused UI element is \d+ button KDL 構文', tree):
            break
        gui.key('Tab')
    else:
        raise AssertionError(tree)
    gui.key('Return')
    state, rows = gui.snapshot('keyboard-readonly-source')
    text = ''.join(row['text'] for row in rows)
    assert '読み取り専用のソース行' in text, text
    assert 'bad' in text, text
    assert gui.state()['text'] == 'Preserved preview', gui.state()
    assert gui.fixture.read_text() == invalid
    records.append(dict(tree=state['snapshot']['treeText'], text=text, input=gui.state(), source=invalid))
    gui.key('Escape')
    state, rows = gui.snapshot('retained-model-after-source')
    assert 'Preserved preview' in ''.join(row['text'] for row in rows)
    assert gui.state()['text'] == 'Preserved preview'
    print('PASS Tab/Return external syntax diagnostic, readonly source, retained preview/draft and original bytes')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
