import argparse, csv, json
from pathlib import Path
import numpy as np
from PIL import Image
parser=argparse.ArgumentParser()
parser.add_argument('--evidence',type=Path,default=Path(__file__).with_name('evidence'))
args=parser.parse_args();root=args.evidence
observations=json.loads((root/'layout-observations.json').read_text())
records={(item['mode'],item['slide']):item for item in observations}
modes=['compact','small','large','fullscreen']
assert set(records)=={(mode,slide) for mode in modes for slide in [1,2]}, 'missing layout observations'
rows=list(csv.reader((root/'glyph-origins.csv').open()))
actual={}
for identity,observation in records.items():
    mode,slide=identity
    viewport=observation['viewport'];expected_scale=observation['scale']
    screenshot=Image.open(root/observation['screenshot'])
    assert abs(screenshot.width/viewport['width']-observation['physicalScale'])<.000001
    frames=[];frame=[]
    for row in rows[observation['traceStart']:observation['traceEnd']]:
        if len(row)!=12:continue
        if (abs(float(row[0])-viewport['width'])>=.000001
            or abs(float(row[1])-viewport['height'])>=.000001
            or abs(float(row[8])-expected_scale)>=.000001
            or abs(float(row[11])-observation['physicalScale'])>=.000001):continue
        text=bytes.fromhex(row[3]).decode()
        if row[2]=='48' and text=='日本語と English の配置比較' and row[4]=='0':
            if frame:frames.append(frame)
            frame=[]
        frame.append(row)
    if frame:frames.append(frame)
    for frame in frames:
        caption=next((r for r in frame if bytes.fromhex(r[3]).decode().startswith('画像の Caption')),None)
        if not caption:continue
        x=(float(caption[6])-float(caption[9]))/float(caption[8])
        if abs(x-({1:652,2:460.24}[slide]))>=.01:continue
        occurrences={};glyphs={}
        for r in frame:
            assert float(r[11])>0, 'physical window scale must be positive'
            if r[4]=='0':occurrences[r[3]]=occurrences.get(r[3],0)+1
            key=(r[3],occurrences[r[3]],r[4])
            assert key not in glyphs
            glyphs[key]=r
        actual[identity]=glyphs
    assert identity in actual, (identity,'no settled glyph frame',observation)

def dilate(a):
    padded=np.pad(a,1);result=np.zeros_like(a)
    for y in range(3):
        for x in range(3):result|=padded[y:y+a.shape[0],x:x+a.shape[1]]
    return result

def stage(mode,slide):
    image=Image.open(root/records[mode,slide]['screenshot']).convert('RGB')
    row=next(iter(actual[mode,slide].values()));physical=float(row[11])
    a=np.array(image);width=float(row[8])*1280*physical
    x=round(float(row[9])*physical)
    white=np.all(a[:,x+2]==255,axis=1);runs=[];start=None
    for i,on in enumerate([*white,False]):
        if on and start is None:start=i
        if not on and start is not None:runs.append((start,i));start=None
    y,end=max(runs,key=lambda pair:pair[1]-pair[0])
    assert abs((end-y)-width*720/1280)<=1
    return image,x,y,width/1280
failures=[]
with (root/'layout-glyph-comparison.csv').open('w',newline='') as f:
    writer=csv.writer(f,lineterminator="\n");writer.writerow(['slide','mode','text_hex','text_occurrence','glyph_index','base_x','base_y','base_error'])
    for slide in [1,2]:
        ref=actual['fullscreen',slide]
        for mode in modes:
            observed=actual[mode,slide]
            assert observed.keys()==ref.keys(),(mode,slide,'missing/reordered glyphs')
            for key,r in observed.items():
                e=ref[key];assert r[5]==e[5],(mode,slide,'glyph mismatch',key)
                scale=float(r[8]);es=float(e[8])
                base=[(float(r[i])-float(r[i+3]))/scale for i in [6,7]]
                expected=[(float(e[i])-float(e[i+3]))/es for i in [6,7]]
                error=max(abs(a-b) for a,b in zip(base,expected))
                writer.writerow([slide,mode,*key,*base,error])
                if error>=.001:failures.append((slide,mode,key,error))
            print(f'{mode} Slide {slide}: {len(observed)} glyphs share base positions')
# Compare strong strokes against weak strokes within one physical pixel in both
# directions. Intensities may differ due to native font antialiasing.
with (root/'layout-raster-comparison.csv').open('w',newline='') as f:
    writer=csv.writer(f,lineterminator="\n");writer.writerow(['slide','mode','element','unmatched_observed','unmatched_reference'])
    for slide in [1,2]:
        full,fx,fy,fs=stage('fullscreen',slide)
        lw=1128*(.5 if slide==1 else .33)
        rects={'heading':(64,48,1152,60),'text':(64,132,1152,126),'bullets':(64,282,lw,92),'code':(64,398,lw,125),'caption':(64+lw+24,614,1128-lw,56)}
        for mode in modes[:-1]:
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
print('PASS: base glyph positions, glyph identities and raster strokes')
