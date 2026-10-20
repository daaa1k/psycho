"""Native write failure retains the edit for retry and document Undo."""
import argparse,json,os,stat
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=root/'target/mvp-save-retry-evidence')
fixture='presentation { metadata { title "GUI dirty"; }; slide { heading "Save retry acceptance"; }; }\n'
gui=Gui(parser.parse_args().output,fixture)
source_dir=gui.out/'source'
mode=None;records=[]
try:
    gui.stop()
    source_dir.mkdir()
    mode=stat.S_IMODE(source_dir.stat().st_mode)
    gui.fixture=source_dir/'presentation.kdl'
    gui.fixture.write_bytes(gui.original)
    gui.start()
    gui.press('Presentation title')
    _,rows=gui.snapshot('save-failure-title-selected')
    fields=[row for row in rows if row['text'] in ('GUI dirty','GUl dirty') and row['x']>1000]
    assert len(fields)==1,rows
    gui.click(fields[0]['x'],fields[0]['y'])
    assert gui.state()['focused'] and gui.state()['text']=='GUI dirty'
    gui.replace('after failure');gui.press('適用')
    expected=gui.original.replace(b'GUI dirty',b'after failure')
    assert gui.fixture.read_bytes()==gui.original
    try:
        os.chmod(gui.fixture.parent,0o500)
        gui.press('保存')
        assert gui.fixture.read_bytes()==gui.original
        gui.button('キャンセル')
    finally:
        os.chmod(gui.fixture.parent,mode)
    text=gui.text('save-failure-keeps-draft')
    assert '保存できませんでした' in text,text
    state=gui.state()
    assert state['text']=='after failure',state
    records.append(dict(name='failure',text=text,input=state,source=gui.fixture.read_text()))
    assert gui.save()==expected,'Retry must save exactly the title edit'
    gui.snapshot('save-failure-retried')
    records.append(dict(name='retry',input=gui.state(),source=gui.fixture.read_text()))
    gui.press('Undo')
    assert gui.save()==gui.original,'One document Undo must restore the original bytes'
    gui.snapshot('save-failure-undo')
    records.append(dict(name='undo',input=gui.state(),source=gui.fixture.read_text()))
    print('PASS failure/retry: source intact; edit/history retained; one Undo restores original bytes',flush=True)
except Exception:
    gui.snapshot('save-failure-error');raise
finally:
    try:
        if mode is not None:os.chmod(source_dir,mode)
        (gui.out/'save-failure-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    finally:gui.stop()
