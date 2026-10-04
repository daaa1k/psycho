import argparse,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-presentation-external-change')
source='presentation { metadata { title "External presentation" }; slide id="one" { heading "First slide"; }; slide id="two" { heading "Second slide"; }; slide id="three" { heading "Third slide"; }; }\n'
external='presentation { metadata { title "External updated" }; slide id="three" { heading "Third updated"; }; slide id="one" { heading "First updated"; }; }\n'
gui=Gui(parser.parse_args().output,source)
try:
    gui.select_slide(2);gui.press('現在から発表');gui.key('Right')
    state,rows=gui.snapshot('external-present-before')
    assert state['screenshot']['width']==1920 and any(row['text']=='Third slide' for row in rows)
    gui.fixture.write_text(external);time.sleep(1)
    state,rows=gui.snapshot('external-present-snapshot')
    assert state['screenshot']['width']==1920 and any(row['text']=='Third slide' for row in rows)
    gui.key('Escape');time.sleep(1)
    state,rows=gui.snapshot('external-return-by-id')
    assert state['screenshot']['width']==1280
    assert any(166<row['x']<995 and row['text']=='Third updated' for row in rows)
    assert gui.fixture.read_text()==external
    gui.press('現在から発表')
    state,rows=gui.snapshot('external-restart-by-id')
    assert state['screenshot']['width']==1920 and any(row['text']=='Third updated' for row in rows)
    gui.key('Right')
    assert any(row['text']=='First updated' for row in gui.snapshot()[1])
    gui.key('Escape');time.sleep(1);gui.hotkey('CmdOrCtrl+Z');gui.press('保存')
    assert gui.fixture.read_text()==external
    print('PASS immutable presentation snapshot, external reorder/delete, return by Slide ID, restart and cleared reload history',flush=True)
except Exception:
    gui.snapshot('external-presentation-failure');raise
finally:gui.stop()
