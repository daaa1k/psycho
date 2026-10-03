"""Capture and compare unfocused and directly edited text in a fresh owned GUI.
Requires numpy/Pillow and Orca. Only changes generated target/mvp-direct files.
"""
import argparse,csv,json,math,shutil,subprocess,time
from pathlib import Path
import numpy as np
from PIL import Image
root=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-direct-evidence');args=parser.parse_args()
out=args.output;out.mkdir(parents=True,exist_ok=True)
work=root/'target/mvp-direct';work.mkdir(parents=True,exist_ok=True)
fixture=work/'layout.kdl';original=(root/'validation/mvp-acceptance/layout.kdl').read_bytes()
if fixture.exists():assert fixture.read_bytes()==original, 'Refusing to replace an edited fixture'
else:fixture.write_bytes(original)
trace=work/'glyphs.csv';trace.write_text('')
import os
environment=dict(os.environ,PSYCHO_GLYPH_TRACE=str(trace))
ocr=root/'target/mvp-recognize-text'
subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_text.swift')),'-o',str(ocr)],check=True)
app=subprocess.Popen([str(root/'target/debug/psycho'),str(fixture)],env=environment,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
def act(*cmd):
    r=json.loads(subprocess.run(['orca','computer',*cmd,'--app',f'pid:{app.pid}','--json'],text=True,capture_output=True).stdout)
    if not r['ok'] and cmd[0]=='click' and r.get('error',{}).get('code')=='window_not_focused':return None
    assert r['ok'],r
    return r['result']
def frame():
    frames=[];current=[]
    for r in csv.reader(trace.open()):
        if len(r)!=12:continue
        if r[2]=='48' and r[4]=='0':
            if current:frames.append(current)
            current=[]
        current.append(r)
    if current:frames.append(current)
    return next(rows for rows in reversed(frames) if len(rows)==len(reference[1]) and abs(float(rows[0][8]) - ((float(rows[0][0])-452)/1280 if rows[0][0]!='1920' else 1.5))<.000001 and all(row[:2]==rows[0][:2] and row[8]==rows[0][8] for row in rows))
def glyphs(rows):
    occurrences={};result={}
    for row in rows:
        if row[4]=='0':occurrences[row[3]]=occurrences.get(row[3],0)+1
        key=(row[3],occurrences[row[3]],row[4]);assert key not in result
        result[key]=row
    return result
reference={};normals=[];checks=[]
def capture(name,rows=None):
    time.sleep(.7);state=act('get-app-state');assert state['screenshot']['width']==(1920 if 'fullscreen' in name else 1440 if 'large' in name else 1280);dest=out/name;shutil.copy(state['screenshot']['path'],dest)
    return dest
# The frame length is discovered from the first complete fullscreen paint.
def full_frame():
    rows=list(csv.reader(trace.open()));start=max(i for i,r in enumerate(rows) if len(r)==12 and r[2]=='48' and r[4]=='0')
    return rows[start:]
def blue_cursor(a):
    a=a.astype(float);return (np.abs(a[:,:,0]-a[:,:,1])<8)&(a[:,:,2]-np.maximum(a[:,:,0],a[:,:,1])>90)
def dilate(a):
    p=np.pad(a,1);result=np.zeros_like(a)
    for y in range(3):
        for x in range(3):result|=p[y:y+a.shape[0],x:x+a.shape[1]]
    return result
try:
    time.sleep(3);act('get-app-state','--restore-window')
    act('click','--x','630','--y','54','--no-screenshot')
    for slide in [1,2]:
        if slide==2:act('press-key','--key','Right','--no-screenshot')
        capture(f'layout-fullscreen-{slide}.png');reference[slide]=full_frame();assert len(reference[slide])>238,len(reference[slide])
        normals.extend(reference[slide])
    act('press-key','--key','Escape','--no-screenshot')
    time.sleep(1.5);assert act('get-app-state')['screenshot']['width']==1280
    for mode,w,h in [('small',1280,892),('large',1440,992)]:
        subprocess.run(['xcrun','swift',str(Path(__file__).with_name('resize_window.swift')),str(app.pid),str(w),str(h)],check=True,capture_output=True)
        for slide in [1,2]:
            act('click','--x','60','--y',str(144+(slide-1)*40),'--no-screenshot')
            capture(f'layout-{mode}-{slide}.png');normal=frame();normals.extend(normal)
            ref=glyphs(reference[slide])
            width=1128*(.5 if slide==1 else .33)
            rois={'heading':(64,48,1152,60),'text':(64,132,1152,126),'bullets':(64,282,width,92),'code':(64,398,width,125),'caption':(64+width+24,614,1128-width,56)}
            for target,prefix in [('heading','日本語と English の配置比較'),('text','日本語と English が混在'),('bullets','箇条書きの行頭'),('code','fn main'),('caption','画像の Caption')]:
                if target!='heading':
                    act('press-key','--key','Escape','--no-screenshot')
                    act('click','--x','60','--y',str(144+(slide-1)*40),'--no-screenshot')
                row=next(r for r in normal if bytes.fromhex(r[3]).decode().startswith(prefix) and r[4]=='0')
                act('click','--x',str(round(float(row[6])+10)),'--y',str(round(float(row[7])+32-5)),'--click-count','2','--no-screenshot')
                file=capture(f'direct-{mode}-{slide}-{target}.png');observed=frame();actual=glyphs(observed);assert actual.keys()==ref.keys(),(mode,slide,target,'missing glyphs')
                max_error=0.
                for key,r in actual.items():
                    expected=ref[key];assert r[5]==expected[5],(mode,slide,target,key,'glyph ID')
                    error=max(abs((float(r[i])-float(r[i+3]))/float(r[8])-(float(expected[i])-float(expected[i+3]))/float(expected[8])) for i in [6,7]);max_error=max(error,max_error)
                assert max_error<.001,(mode,slide,target,max_error)
                text=''.join(subprocess.check_output([str(ocr),str(file)],text=True).split());assert '全文をInspectorで編集' in text,(target,text)
                ox,oy,scale=float(row[9]),float(row[10])+32,float(row[8]);x,y,rw,rh=rois[target]
                box=tuple(round(v) for v in (ox+x*scale,oy+y*scale,ox+(x+rw)*scale,oy+(y+rh)*scale))
                a=np.array(Image.open(file).convert('RGB').crop(box));cursor=blue_cursor(a);assert cursor.sum()>2,(mode,slide,target,'missing cursor')
                e=reference[slide][0];fx,fy,fs=float(e[9]),float(e[10]),float(e[8])
                full=Image.open(out/f'layout-fullscreen-{slide}.png').convert('RGB')
                b=np.array(full.transform((box[2]-box[0],box[3]-box[1]),Image.Transform.EXTENT,(fx+(box[0]-ox)*fs/scale,fy+(box[1]-oy)*fs/scale,fx+(box[2]-ox)*fs/scale,fy+(box[3]-oy)*fs/scale),resample=Image.Resampling.BICUBIC))
                af=a.mean(axis=2);bf=b.mean(axis=2);af[cursor]=255;bf[blue_cursor(b)]=255
                missing_a=int(((af<140)&~dilate(bf<220)).sum());missing_b=int(((bf<140)&~dilate(af<220)).sum());assert missing_a==missing_b==0,(mode,slide,target,missing_a,missing_b)
                checks.append([slide,mode,target,len(observed),max_error,int(cursor.sum()),missing_a,missing_b,'pass'])
                with (out/f'direct-{mode}-{slide}-{target}.csv').open('w',newline='') as f:csv.writer(f,lineterminator='\n').writerows(observed)
                print('PASS',mode,slide,target,flush=True)
            act('press-key','--key','Escape','--no-screenshot')
    assert fixture.read_bytes()==original
    with (out/'glyph-origins.csv').open('w',newline='') as f:csv.writer(f,lineterminator='\n').writerows(normals)
    with (out/'direct-layout-checks.csv').open('w',newline='') as f:
        writer=csv.writer(f,lineterminator='\n');writer.writerow(['slide','mode','target','glyphs','max_base_error','cursor_pixels','unmatched_observed','unmatched_reference','result']);writer.writerows(checks)
finally:
    app.terminate()
    try:app.wait(timeout=5)
    except subprocess.TimeoutExpired:app.kill();app.wait()
