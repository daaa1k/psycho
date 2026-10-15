import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'mvp-acceptance'))
from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
gui = Gui(args.output, 'presentation { metadata { title "Conflict" }; slide { text "Body"; }; }\n')
records = []
try:
    gui.edit('Body', 28)
    gui.press('全文をInspectorで編集')
    _, rows = gui.snapshot()
    field = next(r for r in rows if r['x'] > 1000 and r['text'] == 'Body')
    gui.click(field['x'], field['y'])
    gui.replace('Pending conflict')
    external = gui.fixture.read_text().replace('Conflict', 'External title')
    gui.fixture.write_text(external)
    time.sleep(1.5)
    before = gui.state()
    assert before['text'] == 'Pending conflict'
    gui.key('x')
    gui.hotkey('CmdOrCtrl+Alt+2'); gui.hotkey('CmdOrCtrl+Alt+2')
    gui.hotkey('CmdOrCtrl+Alt+1'); gui.hotkey('CmdOrCtrl+Alt+1')
    gui.hotkey('CmdOrCtrl+1'); gui.hotkey('CmdOrCtrl+0')
    after = gui.state()
    assert after['text'] == before['text'] and after['marked'] == before['marked'], (before, after)
    assert after['undo'] == before['undo'] and after['redo'] == before['redo'], (before, after)
    assert gui.fixture.read_text() == external
    state, _ = gui.snapshot('conflict-input-blocked-view-changes-retain-draft')
    assert '外部ファイルの問題' in gui.text()
    records.append(dict(name='conflict-input-blocked-view-changes-retain-draft', before=before, after=after, tree=state['snapshot']['treeText'], kdl=external))
    print('PASS conflict blocks text input while Inspector, Navigator and zoom preserve retained draft and disk', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
