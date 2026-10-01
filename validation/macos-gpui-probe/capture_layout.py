"""Capture two Slides in two editor sizes and fullscreen using Orca Computer Use."""
import argparse
import json
import shutil
import subprocess
import time
from pathlib import Path
ROOT=Path(__file__).resolve().parent
p=argparse.ArgumentParser()
p.add_argument('--app',required=True)
p.add_argument('--window-id',required=True)
p.add_argument('--orca',default='orca')
p.add_argument('--output',type=Path,default=ROOT/'evidence/shared-layout')
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def action(*args,screenshot=False):
 command=[a.orca,'computer',*args,'--app',a.app,'--window-id',a.window_id,'--json']
 if not screenshot:command.append('--no-screenshot')
 completed=subprocess.run(command, capture_output=True, text=True)
 result=json.loads(completed.stdout)
 if not result.get('ok'):
  if result.get('error',{}).get('code') != 'window_not_focused':raise RuntimeError(result)
  # Native resizing can interrupt the provider's post-click focus check.
  # Observe the actual state; capture() independently checks intended dimensions.
  time.sleep(1.0)
  return action('get-app-state',screenshot=True)
 return result['result']
def capture(mode,slide,width,height):
 time.sleep(.5)
 result=action('get-app-state',screenshot=True)
 image=result['screenshot'];assert (image['width'],image['height'],image['scale'])==(width,height,1),image
 shutil.copyfile(image['path'],a.output/f'layout-{mode}-1-{slide}-live.png')
 print(f'captured {mode}/{slide}: {width}x{height}',flush=True)
action('click','--x','468','--y','124')  # leave title editing at saved initial text
for mode,x,width,height in [('small',60,1024,792),('large',170,1440,992)]:
 action('click','--x',str(x),'--y','83')
 for slide,button in [(1,258),(2,319)]:
  action('click','--x',str(button),'--y','83');capture(mode,slide,width,height)
action('click','--x','108','--y','124')
time.sleep(1.0)
capture('fullscreen',1,1920,1200)
action('press-key','--key','Right');capture('fullscreen',2,1920,1200)
action('press-key','--key','Escape')
shutil.copyfile(ROOT/'target/layout-coordinates.csv',a.output/'layout-coordinates.csv')
shutil.copyfile(ROOT/'target/shared-glyphs.csv',a.output/'glyph-origins.csv')
