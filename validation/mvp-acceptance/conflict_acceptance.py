"""Native external conflict freezes a draft while allowing slide navigation."""
import argparse,json,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=root/'target/mvp-conflict-evidence')
fixture='presentation { metadata { title "GUI dirty"; }; slide { heading "Conflict acceptance"; }; slide { heading "Frozen preview navigation"; }; }\n'
gui=Gui(parser.parse_args().output,fixture)
records=[]
def observe(name):
    text=gui.text(name);state=gui.state()
    records.append(dict(name=name,text=text,input=state,source=gui.fixture.read_text()))
    assert state['text']=='frozen draft',state
    assert gui.fixture.read_bytes()==external
    return text
try:
    gui.press('Presentation title')
    _,rows=gui.snapshot('conflict-title-selected')
    fields=[row for row in rows if row['text'] in ('GUI dirty','GUl dirty') and row['x']>1000]
    assert len(fields)==1,rows
    field=fields[0];gui.click(field['x'],field['y'])
    assert gui.state()['focused'] and gui.state()['text']=='GUI dirty'
    gui.replace('frozen draft')
    _,rows=gui.snapshot('conflict-uncommitted-draft')
    buttons=[row for row in rows if row['text']=='適用']
    assert len(buttons)==1,rows
    apply=buttons[0]
    assert gui.fixture.read_bytes()==gui.original
    external=gui.original.replace(b'GUI dirty',b'external edit')
    gui.fixture.write_bytes(external)
    deadline=time.monotonic()+10
    while True:
        text=gui.text()
        if '競合' in text:break
        assert time.monotonic()<deadline,text
    observe('conflict-freeze-before')
    gui.click(field['x'],field['y'])
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text','--text','should be refused','--no-screenshot')
    observe('conflict-input-refused')
    _,rows=gui.snapshot()
    assert not any(row['text']=='適用' for row in rows),rows
    gui.click(apply['x'],apply['y'])
    observe('conflict-apply-refused')
    gui.hotkey('CmdOrCtrl+S');observe('conflict-save-refused')
    gui.hotkey('CmdOrCtrl+Z');observe('conflict-undo-refused')
    gui.select_slide(2)
    text=observe('conflict-freeze-fixed')
    assert all(value in text for value in ('frozendraft','Frozenpreviewnavigation','GUIdirty')),text
    assert 'shouldberefused' not in text,text
    print('PASS conflict preserves native draft and external bytes through blocked edits/save/Undo and slide navigation',flush=True)
except Exception:
    gui.snapshot('conflict-failure');raise
finally:
    try:(gui.out/'conflict-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    finally:gui.stop()
