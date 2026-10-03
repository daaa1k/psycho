import subprocess,json,time,shutil,argparse
from pathlib import Path
root=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=root/'target/mvp-acceptance')
parser.add_argument('--trace',type=Path,default=root/'target/mvp-glyphs.csv')
args=parser.parse_args();out=args.output;out.mkdir(parents=True,exist_ok=True)
def act(*cmd):
 p=subprocess.run(['orca','computer',*cmd,'--app','psycho','--json'],capture_output=True,text=True)
 r=json.loads(p.stdout)
 if not r['ok']:
  if cmd[0]=='click' and r.get('error',{}).get('code')=='window_not_focused':return
  raise RuntimeError(r)
 return r['result']
def capture(name):
 time.sleep(.6);r=act('get-app-state');shutil.copy(r['screenshot']['path'],out/name);print(name,flush=True)
r=act('get-app-state');pid=r['snapshot']['app']['pid']
for mode,w,h in [('small',1280,892),('large',1440,992)]:
 subprocess.run(['xcrun','swift',str(Path(__file__).with_name('resize_window.swift')),str(pid),str(w),str(h)],check=True,capture_output=True)
 time.sleep(.4)
 for slide in [1,2]:
  act('click','--x','60','--y',str(144+(slide-1)*40),'--no-screenshot');capture(f'layout-{mode}-{slide}.png')
act('click','--x','630','--y','54','--no-screenshot');time.sleep(1)
capture('layout-fullscreen-1.png')
act('press-key','--key','Right','--no-screenshot');capture('layout-fullscreen-2.png')
act('press-key','--key','Escape','--no-screenshot');time.sleep(1)
shutil.copy(args.trace,out/'glyph-origins.csv')
