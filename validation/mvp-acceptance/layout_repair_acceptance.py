import argparse,json,subprocess,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-layout-repair')
large='overflow();\n'*30
short='repaired();'
source='presentation { metadata { title "Layout repair" }; slide id="good" { heading "Good slide"; }; slide id="bad" { heading "Offscreen code"; code '+json.dumps(large)+'; }; }\n'
gui=Gui(parser.parse_args().output,source)
try:
    assert gui.save()==gui.original,'Overflowing content remains saveable'
    for label in ['最初から発表','現在から発表']:
        gui.press(label)
        state,rows=gui.snapshot('noncurrent-blocked-'+str(len(label)))
        assert state['screenshot']['width']==1280
        assert any(166<row['x']<995 and row['text']=='Good slide' for row in rows)
    _,rows=gui.snapshot()
    row=next(row for row in rows if 'Layout' in row['text'] and 'Element' in row['text'])
    gui.click(row['x'],row['y'])
    assert gui.state()['text']==large,'Diagnostic must select the complete overflowing Element'
    _,rows=gui.snapshot('offscreen-full-inspector')
    row=next(row for row in rows if row['x']>1000 and 'overflow' in row['text'])
    gui.click(row['x'],row['y']);gui.hotkey('CmdOrCtrl+A');gui.hotkey('CmdOrCtrl+C')
    assert subprocess.check_output(['pbpaste']).decode()==large,'Inspector must retain the whole offscreen value'
    gui.replace(short)
    _,rows=gui.snapshot('overflow-repaired-preview')
    assert not any('Layout' in row['text'] and 'Element' in row['text'] for row in rows)
    assert gui.fixture.read_bytes()==gui.original,'Preview repair must not save automatically'
    saved=gui.save();assert saved.decode()==source.replace(json.dumps(large),json.dumps(short))
    gui.press('Undo');assert gui.save()==gui.original
    gui.press('Redo');assert gui.save()==saved
    gui.press('最初から発表');gui.key('Right')
    state,rows=gui.snapshot('repaired-presentation')
    assert state['screenshot']['width']==1920 and any('repaired' in row['text'] for row in rows)
    assert gui.fixture.read_bytes()==saved
    gui.key('Escape');time.sleep(1)
    print('PASS noncurrent overflow blocks both starts, full Inspector repair, immediate diagnosis clearing, Save, exact Undo/Redo and presentation',flush=True)
except Exception:
    gui.snapshot('layout-repair-failure');raise
finally:gui.stop()
