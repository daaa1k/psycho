import json, os, re, subprocess, sys
from pathlib import Path
root=Path.cwd();sys.path.insert(0,str(root/'validation/mvp-acceptance'))
from gui_session import Gui
out=root/'target/inspector-focus-probe-8f9d900';out.mkdir()
os.environ['PSYCHO_EDITOR_STATE']=str(out/'editor.json')
gui=Gui(out,'presentation { metadata { title "Focus probe"; }; slide { heading "Slide"; }; }')
records=[]
def observe(label):
 state,rows=gui.snapshot(label);records.append(dict(name=label,input=gui.state(),tree=state['snapshot']['treeText']))
 (out/'results.json').write_text(json.dumps(records,ensure_ascii=False,indent=2));return state,rows
try:
 tree=gui.act('get-app-state')['snapshot']['treeText']
 index=re.search(r'^\s*(\d+) button Presentation title$',tree,re.M)[1]
 gui.act('click','--element-index',index,'--no-screenshot')
 snapshot,rows=observe('before-coordinate')
 matches=[r for r in rows if r['text']=='Focus probe' and r['x']>snapshot['snapshot']['window']['width']-300]
 assert len(matches)==1,rows
 row=matches[0];gui.click(row['x'],row['y']);observe('after-orca-coordinate')
 helper=out/'native-double-click'
 subprocess.run(['xcrun','swiftc',str(root/'validation/mvp-acceptance/native_double_click.swift'),'-o',str(helper)],check=True)
 subprocess.run([str(helper),str(gui.app.pid),str(row['x']),str(row['y'])],check=True)
 observe('after-native-double-click')
 print(json.dumps(records,ensure_ascii=False))
 assert gui.state()['focused'],gui.state()
 gui.replace('Native focus verified');observe('native-replaced')
finally:gui.stop()
