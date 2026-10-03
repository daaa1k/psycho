"""Check shared base glyph positions and raster strokes in three real GUI sizes.
Requires numpy and Pillow. Exits nonzero on any failed check.
"""
import argparse, csv, math
from pathlib import Path
import numpy as np
from PIL import Image
parser=argparse.ArgumentParser()
parser.add_argument('--evidence',type=Path,default=Path(__file__).with_name('evidence'))
args=parser.parse_args();root=args.evidence
frames=[];frame=[]
for row in csv.reader((root/'glyph-origins.csv').open()):
    if len(row)!=12:continue
    text=bytes.fromhex(row[3]).decode()
    if row[2]=='48' and text=='日本語と English の配置比較' and row[4]=='0':
        if frame:frames.append(frame)
        frame=[]
    frame.append(row)
if frame:frames.append(frame)
actual={}
for frame in frames:
    caption=next((r for r in frame if bytes.fromhex(r[3]).decode().startswith('画像の Caption')),None)
    if not caption:continue
    mode={1280:'small',1440:'large',1920:'fullscreen'}.get(round(float(caption[0])))
    if not mode:continue
    x=(float(caption[6])-float(caption[9]))/float(caption[8])
    slide=1 if abs(x-652)<.01 else 2 if abs(x-460.24)<.01 else None
    expected_scale=(float(caption[0])-452)/1280 if mode!='fullscreen' else 1.5
    # Resize/fullscreen transitions can paint an old layout at a new viewport.
    # Use complete settled frames, matching the dimensions of saved screenshots.
    if any(r[0:2]!=caption[0:2] or abs(float(r[8])-expected_scale)>=.000001 for r in frame):continue
    if slide:
        occurrences={};glyphs={}
        for r in frame:
            assert float(r[11]) == 1, 'acceptance requires physical window scale 1'
            expected_scale=(float(r[0])-452)/1280 if mode!='fullscreen' else 1.5
            assert abs(float(r[8])-expected_scale)<.000001
            if r[4]=='0':occurrences[r[3]]=occurrences.get(r[3],0)+1
            key=(r[3],occurrences[r[3]],r[4])
            assert key not in glyphs
            glyphs[key]=r
        actual[mode,slide]=glyphs

def dilate(a):
    padded=np.pad(a,1);result=np.zeros_like(a)
    for y in range(3):
        for x in range(3):result|=padded[y:y+a.shape[0],x:x+a.shape[1]]
    return result

def stage(mode,slide):
    image=Image.open(root/f'layout-{mode}-{slide}.png').convert('RGB')
    a=np.array(image);width=image.width if mode=='fullscreen' else image.width-452
    x=0 if mode=='fullscreen' else 166
    white=np.all(a[:,x+2]==255,axis=1);runs=[];start=None
    for i,on in enumerate([*white,False]):
        if on and start is None:start=i
        if not on and start is not None:runs.append((start,i));start=None
    y,end=max(runs,key=lambda pair:pair[1]-pair[0])
    assert abs((end-y)-width*720/1280)<=1
    return image,x,y,width/1280
failures=[]
with (root/'layout-glyph-comparison.csv').open('w',newline='') as f:
    writer=csv.writer(f,lineterminator="\n");writer.writerow(['slide','mode','text_hex','text_occurrence','glyph_index','base_x','base_y','base_error','quantization_error_px'])
    for slide in [1,2]:
        ref=actual['fullscreen',slide]
        for mode in ['small','large','fullscreen']:
            observed=actual[mode,slide]
            assert observed.keys()==ref.keys(),(mode,slide,'missing/reordered glyphs')
            for key,r in observed.items():
                e=ref[key];assert r[5]==e[5],(mode,slide,'glyph mismatch',key)
                scale=float(r[8]);es=float(e[8])
                base=[(float(r[i])-float(r[i+3]))/scale for i in [6,7]]
                expected=[(float(e[i])-float(e[i+3]))/es for i in [6,7]]
                error=max(abs(a-b) for a,b in zip(base,expected))
                # GPUI quantizes glyph x to quarter-pixels, y to whole pixels.
                quant=max(abs(math.floor(float(r[6])*4)/4-float(r[6])),abs(math.floor(float(r[7]))-float(r[7])))
                writer.writerow([slide,mode,*key,*base,error,quant])
                if error>.001 or quant>1:failures.append((slide,mode,key,error,quant))
            print(f'{mode} Slide {slide}: {len(observed)} glyphs share base positions')
# Compare strong strokes against weak strokes within one physical pixel in both
# directions. Intensities may differ due to native font antialiasing.
with (root/'layout-raster-comparison.csv').open('w',newline='') as f:
    writer=csv.writer(f,lineterminator="\n");writer.writerow(['slide','mode','element','unmatched_observed','unmatched_reference'])
    for slide in [1,2]:
        full,fx,fy,fs=stage('fullscreen',slide)
        lw=1128*(.5 if slide==1 else .33)
        rects={'heading':(64,48,1152,60),'text':(64,132,1152,126),'bullets':(64,282,lw,92),'code':(64,398,lw,125),'caption':(64+lw+24,614,1128-lw,56)}
        for mode in ['small','large']:
            image,ox,oy,scale=stage(mode,slide)
            for element,(x,y,w,h) in rects.items():
                box=tuple(round(v) for v in (ox+x*scale,oy+y*scale,ox+(x+w)*scale,oy+(y+h)*scale))
                observed=np.array(image.crop(box)).mean(axis=2)
                reference=full.transform((box[2]-box[0],box[3]-box[1]),Image.Transform.EXTENT,(fx+(box[0]-ox)*fs/scale,fy+(box[1]-oy)*fs/scale,fx+(box[2]-ox)*fs/scale,fy+(box[3]-oy)*fs/scale),resample=Image.Resampling.BICUBIC)
                expected=np.array(reference).mean(axis=2)
                missing_observed=int(((observed<140)&~dilate(expected<220)).sum())
                missing_reference=int(((expected<140)&~dilate(observed<220)).sum())
                writer.writerow([slide,mode,element,missing_observed,missing_reference])
                if missing_observed or missing_reference:failures.append((slide,mode,element,missing_observed,missing_reference))
assert not failures,failures
print('PASS: base glyph positions, glyph identities, quantization and raster strokes')
