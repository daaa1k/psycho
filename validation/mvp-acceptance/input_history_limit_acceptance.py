import argparse,subprocess,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-input-history-limit')
source='presentation { metadata { title "Input limit" }; slide { code "body"; }; }\n'
gui=Gui(parser.parse_args().output,source)
keys=gui.out/'history-keys'
def repeat(key,count):
    gui.act('get-app-state','--restore-window')
    subprocess.run([str(keys),str(gui.app.pid),key,str(count)],check=True)
    time.sleep(1)
try:
    subprocess.run(['xcrun','swiftc',str(root/'validation/mvp-acceptance/history_keys.swift'),'-o',str(keys)],check=True)
    gui.edit('body',22);gui.key('End')
    subprocess.run(['pbcopy'],input=b'a',check=True)
    repeat('paste',1001)
    state=gui.state();assert state['text']=='body'+'a'*1001,state
    assert state['undo']==1000,('Input history must discard the oldest operation',state['undo'])
    print('PASS input retains exactly the latest 1,000 of 1,001 pasted units',flush=True)
    repeat('undo',1000)
    assert gui.state()['text']=='bodya' and gui.state()['undo']==0 and gui.state()['redo']==1000
    print('PASS local Undo stops at the surviving baseline with 1,000 Redo units',flush=True)
    gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']=='bodya'
    gui.key('Escape')
    first=gui.save();assert b'code "bodya"' in first,'Commit must retain the oldest surviving input baseline'
    gui.hotkey('CmdOrCtrl+Shift+Z');assert b'code "bodyaa"' in gui.save()
    gui.hotkey('CmdOrCtrl+Z');assert gui.save()==first
    gui.hotkey('CmdOrCtrl+Z');assert gui.save()==first,'Global Undo must stop at the same 1,000-unit baseline'
    repeat('redo',1000)
    assert gui.save().decode()==source.replace('"body"','"body'+'a'*1001+'"')
    print('PASS all 1,000 global Redo units restore the final input',flush=True)
    repeat('undo',1000);assert gui.save()==first
    gui.snapshot('input-limit-baseline')
    print('PASS 1,000-unit input limit, all-undone commit, full Redo and preserved baseline',flush=True)
except Exception:
    gui.snapshot('input-limit-failure');raise
finally:gui.stop()
