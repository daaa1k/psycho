"""Native Save As failure must retain the source, draft and history for retry."""
import argparse,json,os,stat
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-save-as-retry')
gui=Gui(parser.parse_args().output,
    'presentation { metadata { title "Save As retry" }; slide { heading "Original"; }; }\n')
archive=gui.out/'archive';archive.mkdir();mode=stat.S_IMODE(archive.stat().st_mode)
dest=archive/'presentation.kdl';records=[]
def native_button(name):
    state=gui.act('get-app-state')
    line=next(line for line in state['snapshot']['treeText'].splitlines() if line.strip().endswith('button '+name))
    gui.act('click','--element-index',line.strip().split()[0],'--no-screenshot')
def choose_destination():
    gui.press('別名保存');native_button('退避先を選ぶ');gui.hotkey('CmdOrCtrl+Shift+G');gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text','--text',str(archive),'--no-screenshot');gui.key('Return')
    assert 'Where:, Value: archive' in gui.act('get-app-state')['snapshot']['treeText']
try:
    gui.edit('Original',48);gui.replace('Retained after failure')
    choose_destination();os.chmod(archive,0o500)
    native_button('Save')
    state,rows=gui.snapshot('save-as-failed')
    text=''.join(row['text'] for row in rows)
    records.append(dict(name='failure',text=text,tree=state['snapshot']['treeText']))
    assert gui.fixture.read_bytes()==gui.original and not dest.exists()
    assert '退避保存できません' in text,records[-1]
    assert gui.state()['text']=='Retained after failure'
    gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']=='Original'
    gui.hotkey('CmdOrCtrl+Shift+Z');assert gui.state()['text']=='Retained after failure'
    assert gui.fixture.read_bytes()==gui.original
    os.chmod(archive,mode)
    choose_destination();native_button('Save')
    state,rows=gui.snapshot('save-as-retried')
    assert b'heading "Retained after failure"' in dest.read_bytes()
    assert gui.fixture.read_bytes()==gui.original
    saved=dest.read_bytes();gui.hotkey('CmdOrCtrl+Z');gui.press('保存')
    assert dest.read_bytes()==saved, 'Successful Save As clears history'
    assert gui.fixture.read_bytes()==gui.original
    records.append(dict(name='retry',source=dest.read_text(),text=''.join(row['text'] for row in rows)))
    print('PASS Native Save As write failure, retained source/draft, retry and history reset',flush=True)
finally:
    os.chmod(archive,mode)
    (gui.out/'save-as-retry-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
