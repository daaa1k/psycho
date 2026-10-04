import json,subprocess,time,shutil,os,argparse,shlex,stat
from pathlib import Path
root=Path(__file__).resolve().parents[2]
p=root/'target/mvp-save-retry/presentation.kdl'
fixture='presentation { metadata { title "GUI dirty"; }; slide { heading "Save retry acceptance"; }; }\n'
parser=argparse.ArgumentParser(description='Owned scratch document: GUI write failure and retry regression')
parser.add_argument('--prepare',action='store_true')
parser.add_argument('--output',type=Path,default=root/'target/mvp-save-retry-evidence')
args=parser.parse_args()
if args.prepare:
    p.parent.mkdir(parents=True,exist_ok=True)
    if p.exists():assert p.read_text()==fixture, 'Refusing to replace an edited fixture'
    else:p.write_text(fixture)
    print(f'./target/debug/psycho {shlex.quote(str(p))}')
    raise SystemExit(0)
assert p.read_text()==fixture, 'Run --prepare and open the scratch fixture'
out=args.output;out.mkdir(parents=True,exist_ok=True)
before=p.read_bytes();directory_mode=stat.S_IMODE(p.parent.stat().st_mode)
def act(*cmd):
 r=json.loads(subprocess.check_output(['orca','computer',*cmd,'--app','psycho','--json'],text=True));assert r['ok'],r
 return r['result']
def snap(name):
 time.sleep(1);r=act('get-app-state');shutil.copy(r['screenshot']['path'],out/name);print(name,flush=True)
state=act('get-app-state')
pid=state['snapshot']['app']['pid']
command=shlex.split(subprocess.check_output(['ps','-p',str(pid),'-o','command='],text=True).strip())
assert command[-1]==str(p), 'psycho must have the owned scratch document open'
assert Path(command[0]).resolve()==root/'target/debug/psycho'
assert state['snapshot']['window']['width']==1280, 'Use the initial 1280px editor'
act('click','--x','1140','--y','148','--no-screenshot')
act('click','--x','1100','--y','231','--no-screenshot');act('hotkey','--key','CmdOrCtrl+A','--no-screenshot');act('type-text','--text','after failure','--no-screenshot');act('click','--x','1140','--y','278','--no-screenshot')
try:
 os.chmod(p.parent,0o500)
 act('click','--x','84','--y','54','--no-screenshot');snap('save-failure-keeps-draft.png')
 assert p.read_bytes()==before
finally:os.chmod(p.parent,directory_mode)
act('click','--x','84','--y','54','--no-screenshot');snap('save-failure-retried.png')
assert 'title "after failure"' in p.read_text()
act('click','--x','214','--y','54','--no-screenshot');act('click','--x','84','--y','54','--no-screenshot');snap('save-failure-undo.png')
assert p.read_bytes()==before

print('PASS failure/retry: source intact; dirty input/history retained; one Undo restores saved bytes',flush=True)
