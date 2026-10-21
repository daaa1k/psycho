"""Verify popup Escape returns to the text field and Tab reaches editing."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'mvp-acceptance'))
from gui_session import Gui
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
os.environ['PSYCHO_EDITOR_STATE']=str(args.output.resolve()/'editor-state.json')
gui=Gui(args.output,'presentation { metadata { title "Focus" }; slide { text "Original body"; }; }\n')
records=[]
def capture(name):
 s=gui.act('get-app-state');shutil.copy(s['screenshot']['path'],gui.out/f'{name}.png')
 records.append(dict(name=name,input=gui.state(),tree=s['snapshot']['treeText'],editor=json.loads((gui.out/'editor-state.json').read_text())))
 (gui.out/'observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2))
try:
 gui.edit('Original body',28)
 assert gui.state()['focused']
 capture('input-before-popup')
 tree=gui.act('get-app-state')['snapshot']['treeText']
 control=re.search(r'^\s*(\d+) button 表示$',tree,re.MULTILINE)
 assert control,tree
 gui.act('click','--element-index',control[1],'--no-screenshot')
 assert not gui.state()['focused']
 capture('popup-root-focus')
 gui.key('Escape')
 assert gui.state()['focused']
 capture('escape-restores-input')
 assert gui.fixture.read_bytes()==gui.original
 gui.press('全文をInspectorで編集')
 capture('inspector-editor-before-tab')
 gui.hotkey('Shift+Tab')
 for number in range(40):
  gui.key('Tab')
  gui.act('type-text','--text','x','--no-screenshot')
  capture(f'tab-{number:02d}')
  if gui.state()['focused']:break
 else:raise AssertionError('Tab did not reach the edit field')
 assert 'text field (settable) 本文' in gui.act('get-app-state')['snapshot']['treeText']
 gui.replace('Keyboard body')
 gui.hotkey('CmdOrCtrl+S')
 assert b'text "Keyboard body"' in gui.fixture.read_bytes()
 capture('tab-edit-keyboard-save')
 print('PASS popup Escape restores input and Tab reaches editing with keyboard Save',flush=True)
finally:gui.stop()
