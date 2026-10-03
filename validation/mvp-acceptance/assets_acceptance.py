"""Real GUI checks for EXIF, alpha and Save As reference preservation.
Uses a fresh, owned psycho process and only generated files under target/.
Requires Pillow (the same venv as measure_layout.py).
"""
import argparse,csv,json,shutil,subprocess,time
from io import BytesIO
from pathlib import Path
from PIL import Image,ImageCms
root=Path(__file__).resolve().parents[2]
p=root/'target/mvp-assets/presentation.kdl'
fixture='''presentation {
 metadata { title "Asset acceptance" }
 slide { heading "EXIF orientation"; image "orientation-6.jpg" { caption "Red above blue"; }; }
 slide { heading "Alpha transparency"; image "alpha.png" { caption "White / pink / blue"; }; }
 slide { heading "WebP transparency"; image "still.webp" { caption "White / pink / blue"; }; }
}
'''
parser=argparse.ArgumentParser()
parser.add_argument('--prepare',action='store_true')
parser.add_argument('--output',type=Path,default=root/'target/mvp-asset-evidence')
args=parser.parse_args()
if args.prepare:
    p.parent.mkdir(parents=True,exist_ok=True)
    if p.exists():assert p.read_text()==fixture, 'Refusing to replace an edited fixture'
    else:p.write_text(fixture)
    jpg=(root/'tests/fixtures/orientation-6.jpg').read_bytes()
    a=Image.new('RGBA',(30,20),(255,0,0,0));a.paste((255,0,0,128),(10,0,20,20));a.paste((0,0,255,255),(20,0,30,20))
    files={'orientation-6.jpg':jpg}
    for name,format in [('alpha.png','PNG'),('still.webp','WEBP')]:
        data=BytesIO();a.save(data,format=format,**({'lossless':True} if format=='WEBP' else {}));files[name]=data.getvalue()
    for name,data in files.items():
        dest=p.parent/name
        if dest.exists():assert dest.read_bytes()==data, f'Refusing to replace a changed asset: {dest}'
        else:dest.write_bytes(data)
    print('Prepared owned assets. Run this script without --prepare; it launches its own app.')
    raise SystemExit(0)
assert p.read_text()==fixture, 'Run --prepare first'
# AX cannot read application windows at the login screen. Fail before replacing evidence.
locked=subprocess.check_output(['swift','-e','import Cocoa; print((CGSessionCopyCurrentDictionary() as? [String: Any])?["CGSSessionScreenIsLocked"] as? Bool ?? false)'],text=True).strip()
assert locked=='false', 'Unlock the Mac before running GUI acceptance'

out=args.output;out.mkdir(parents=True,exist_ok=True)
archive=p.parent/'archive/deep';archive.mkdir(parents=True,exist_ok=True)
dest=archive/'presentation.kdl'
if dest.exists():assert 'title "Asset acceptance"' in dest.read_text(), 'Destination must be an owned previous test artifact'
ocr=root/'target/mvp-recognize-text'
subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_text.swift')),'-o',str(ocr)],check=True)
app=subprocess.Popen([str(root/'target/debug/psycho'),str(p)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
def act(*cmd):
    result=subprocess.run(['orca','computer',*cmd,'--app',f'pid:{app.pid}','--json'],text=True,capture_output=True)
    r=json.loads(result.stdout)
    if not r['ok']:
        # Native confirmation transitions may deliver a click then change focus.
        # The caller observes the resulting dialog/editor before doing anything else.
        if cmd[0]=='click' and r.get('error',{}).get('code')=='window_not_focused':return None
        raise RuntimeError(r)
    return r['result']
def snapshot(name):
    time.sleep(.7);state=act('get-app-state');shutil.copy(state['screenshot']['path'],out/name)
    return state,out/name
def srgb(file):
    image=Image.open(file)
    if image.info.get('icc_profile'):
        return ImageCms.profileToProfile(image,ImageCms.ImageCmsProfile(BytesIO(image.info['icc_profile'])),ImageCms.createProfile('sRGB'),outputMode='RGB')
    return image.convert('RGB')
def press_button(name):
    state=act('get-app-state');line=next(line for line in state['snapshot']['treeText'].splitlines() if line.strip().endswith(f'button {name}'))
    act('click','--element-index',line.strip().split()[0],'--no-screenshot')
try:
    time.sleep(3)
    act('get-app-state','--restore-window')
    with (out/'asset-render-colors.csv').open('w',newline='') as f:
        writer=csv.writer(f,lineterminator='\n');writer.writerow(['slide','format','point','x','y','red','green','blue','result'])
        for slide,format,points in [(1,'JPEG',[(580,350,'red'),(580,460,'blue')]),(2,'PNG',[(477,410,'white'),(580,410,'pink'),(683,410,'blue')]),(3,'WebP',[(477,410,'white'),(580,410,'pink'),(683,410,'blue')])]:
            act('click','--x','65','--y',str(144+(slide-1)*40),'--no-screenshot')
            state,file=snapshot(f'asset-render-{format.lower()}.png');assert state['screenshot']['width']==1280
            heading={1:'EXIForientation',2:'Alphatransparency',3:'WebPtransparency'}[slide]
            text=''.join(subprocess.check_output([str(ocr),str(file)],text=True).split());assert heading in text,(slide,text)
            image=srgb(file)
            for x,y,expected in points:
                red,green,blue=image.getpixel((x,y))
                ok={'red':red>220 and green<30 and blue<30,'blue':blue>220 and red<30 and green<30,'white':min(red,green,blue)>245,'pink':red>240 and 70<green<230 and abs(green-blue)<5}[expected]
                writer.writerow([slide,format,expected,x,y,red,green,blue,'pass' if ok else 'fail']);f.flush();assert ok,(format,expected,red,green,blue)
            print('PASS',format,flush=True)
    act('click','--x','683','--y','410','--no-screenshot') # select image path
    act('click','--x','145','--y','54','--no-screenshot')
    act('hotkey','--key','CmdOrCtrl+Shift+G','--no-screenshot')
    act('hotkey','--key','CmdOrCtrl+A','--no-screenshot')
    act('type-text','--text',str(archive),'--no-screenshot')
    act('press-key','--key','Return','--no-screenshot')
    state=act('get-app-state');assert 'Where:, Value: deep' in state['snapshot']['treeText']
    press_button('Save')
    state=act('get-app-state')
    if 'button Replace' in state['snapshot']['treeText']:press_button('Replace')
    state,file=snapshot('asset-retreat-synced-input.png');assert state['screenshot']['width']==1280
    saved=dest.read_bytes()
    for name in ['orientation-6.jpg','alpha.png','still.webp']:
        assert f'image "../../{name}"' in saved.decode() and (archive/'../..'/name).exists()
    text=subprocess.check_output([str(ocr),str(file)],text=True);assert '未保存' not in text,text
    act('click','--x','84','--y','54','--no-screenshot')
    state,file=snapshot('asset-retreat-resaved.png')
    assert dest.read_bytes()==saved and p.read_text()==fixture
    assert srgb(file).getpixel((683,410))[2]>220
    print('PASS retreat/resave: all rebased references and original bytes preserved',flush=True)
finally:
    app.terminate()
    try:app.wait(timeout=5)
    except subprocess.TimeoutExpired:app.kill();app.wait()
