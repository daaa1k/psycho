import json,subprocess,time,shutil,argparse,shlex
from pathlib import Path
root=Path(__file__).resolve().parents[2]
p=root/'target/mvp-conflict-freeze/presentation.kdl'
fixture='presentation { metadata { title "GUI dirty"; }; slide { heading "Conflict acceptance"; }; slide { heading "Frozen preview navigation"; }; }\n'
parser=argparse.ArgumentParser(description='Owned scratch document: freeze edits during external conflict')
parser.add_argument('--prepare',action='store_true')
parser.add_argument('--output',type=Path,default=root/'target/mvp-conflict-evidence')
args=parser.parse_args()
if args.prepare:
 p.parent.mkdir(parents=True,exist_ok=True)
 if p.exists():assert p.read_text()==fixture, 'Refusing to replace an edited fixture'
 else:p.write_text(fixture)
 print(f'./target/debug/psycho {shlex.quote(str(p))}')
 raise SystemExit(0)
assert p.read_text()==fixture, 'Run --prepare and open the scratch fixture'
before=p.read_bytes();out=args.output;out.mkdir(parents=True,exist_ok=True)
ocr=root/'target/mvp-recognize-text'
subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_text.swift')),'-o',str(ocr)],check=True)
def act(*cmd):
 r=json.loads(subprocess.check_output(['orca','computer',*cmd,'--app','psycho','--json'],text=True));assert r['ok'],r
 return r['result']
def type_value(value):
 act('click','--x','1100','--y','231','--no-screenshot');act('hotkey','--key','CmdOrCtrl+A','--no-screenshot');act('type-text','--text',value,'--no-screenshot')
def snap(name):
 time.sleep(1);r=act('get-app-state');shutil.copy(r['screenshot']['path'],out/name);print(name,flush=True)
state=act('get-app-state')
pid=state['snapshot']['app']['pid']
command=shlex.split(subprocess.check_output(['ps','-p',str(pid),'-o','command='],text=True).strip())
assert command[-1]==str(p) and Path(command[0]).resolve()==root/'target/debug/psycho', 'Open the owned scratch fixture'
assert state['snapshot']['window']['width']==1280
try:
 type_value('frozen draft')
 p.write_bytes(before.replace(b'GUI dirty',b'external edit'))
 snap('conflict-freeze-before.png')
 type_value('should be refused')
 act('click','--x','1140','--y','278','--no-screenshot')
 act('hotkey','--key','CmdOrCtrl+S','--no-screenshot')
 act('hotkey','--key','CmdOrCtrl+Z','--no-screenshot')
 act('click','--x','65','--y','183','--no-screenshot')
 snap('conflict-freeze-fixed.png')
 assert p.read_bytes()==before.replace(b'GUI dirty',b'external edit')
 text=subprocess.check_output([str(ocr),str(out/'conflict-freeze-fixed.png')],text=True)
 text=''.join(text.split())
 assert all(value in text for value in ['frozendraft','Frozenpreviewnavigation','GUIdirty']) and 'shouldberefused' not in text,text
 print('PASS: conflict preserves draft, ignores editor/save/undo actions',flush=True)
finally:p.write_bytes(before)
