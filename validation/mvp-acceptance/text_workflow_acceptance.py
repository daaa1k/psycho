"""Verify raw code tabs, surface switching, history transfer, Save and reload."""
import argparse,json,subprocess
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-text-workflow')
out=parser.parse_args().output
old='fn\tmain() {\n\tprintln!("日本語");\n}'
new='fn\tmain() {\n\tlet 値 = 7;\n\tprintln!("{}", 値);\n}\t'
quote=lambda text:json.dumps(text,ensure_ascii=False)
source=f'presentation {{ metadata {{ title "Text workflow" }}; slide {{ heading "Code workflow"; code language="rust" {quote(old)}; }}; }}\n'
gui=Gui(out,source)
records=[]
def record(name):
    value=gui.state();records.append(dict(name=name,input=value));gui.snapshot(name)
    print(name,json.dumps(value,ensure_ascii=False),flush=True)
    return value
try:
    gui.edit('fn',22)
    initial=record('code-direct-original');assert initial['text']==old and initial['focused']
    gui.press('全文をInspectorで編集')
    inspector=record('code-inspector-original')
    assert inspector['text']==old and inspector['undo']==initial['undo']
    gui.hotkey('CmdOrCtrl+A');gui.hotkey('CmdOrCtrl+C')
    assert subprocess.check_output(['pbpaste']).decode()==old, 'Copy must preserve raw tabs and Unicode'
    gui.hotkey('CmdOrCtrl+X');cut=record('code-cut');assert cut['text']=='' and cut['undo']==1
    gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']==old and gui.state()['undo']==0
    gui.replace(new)
    edited=record('code-inspector-edited');assert edited['marked']==[-1,-1] and edited['undo']==1
    gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']==old
    gui.hotkey('CmdOrCtrl+Shift+Z');assert gui.state()['text']==new
    gui.edit('fn',22)
    direct=record('code-return-to-canvas')
    for key in ['text','selected','marked','undo','redo']:
        assert direct[key]==edited[key],(key,direct,edited)
    assert direct['cursor'][0]<1000 and direct['focused']
    assert gui.fixture.read_bytes()==gui.original,'Editing must not save automatically'
    steps=edited['undo'];gui.key('Escape')
    saved=gui.save();assert saved.decode()==source.replace(quote(old),quote(new)),saved
    gui.snapshot('code-explicit-save')
    for step in range(steps):gui.press('Undo')
    assert gui.save()==gui.original,'Global Undo must restore all original KDL bytes'
    gui.snapshot('code-global-undo')
    for step in range(steps):gui.press('Redo')
    assert gui.save()==saved,'Global Redo must restore the same raw tabs and code'
    gui.snapshot('code-global-redo')
    gui.press('コード言語')
    assert gui.state()['text']=='rust'
    gui.replace('python');gui.press('適用')
    language=gui.save();assert language==saved.replace(b'language="rust"',b'language="python"')
    gui.press('Undo');assert gui.save()==saved
    gui.stop();gui.start()
    gui.edit('fn',22);reloaded=record('code-reloaded')
    assert reloaded['text']==new and reloaded['undo']==0,'Fresh GUI must reload the saved tabs'
    gui.key('Tab');assert gui.state()['text']==new+'\t' and gui.state()['undo']==1
    gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']==new and gui.state()['undo']==0
    gui.key('Escape');gui.press('現在から発表')
    state,rows=gui.snapshot('code-presentation')
    assert state['screenshot']['width']==1920 and any('Code workflow' in r['text'] for r in rows)
    assert gui.fixture.read_bytes()==saved,'Presentation must not save automatically'
    gui.key('Escape')
    print('PASS code tabs, Canvas/Inspector roundtrip, input history transfer, Save, Undo/Redo, language and GUI reload',flush=True)
finally:
    (gui.out/'text-workflow-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
