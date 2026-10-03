"""Check actual image/code rectangles following empty/tall columns and captions."""
import argparse,csv,json
from io import BytesIO
from pathlib import Path
import numpy as np
from PIL import Image,ImageCms
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-following-layout')
source='''presentation {
 metadata { title "Following layout" }
 slide { heading "Empty columns"; columns left=50 right=50 { column {}; column {}; }; image "marker.png"; }
 slide { heading "Tall columns"; columns left=50 right=50 { column { text "First\\nSecond\\nThird"; }; column { text "Short"; }; }; image "marker.png"; }
 slide { heading "No caption"; image "marker.png"; code "marker();"; }
 slide { heading "Empty caption"; image "marker.png" { caption ""; }; code "marker();"; }
}
'''
gui=Gui(parser.parse_args().output,source);records=[]
try:
    Image.new('RGB',(256,128),(255,0,255)).save(gui.out/'marker.png')
    gui.stop();gui.start()
    for i,(heading,kind,base_y) in enumerate([
        ('Empty columns','image',156),('Tall columns','image',282),
        ('No caption','code',476),('Empty caption','code',476)]):
        if i:gui.click(65,142+i*42)
        for mode in ['editor','presentation']:
            if mode=='presentation':gui.press('現在から発表')
            state,rows=gui.snapshot(f'following-{i+1}-{mode}')
            assert any(heading in row['text'] for row in rows),(heading,rows)
            trace=list(csv.reader(gui.trace.open()))
            row=next(row for row in reversed(trace) if len(row)==12 and bytes.fromhex(row[3]).decode()==heading)
            scale=float(row[8]);origin_y=float(row[10])+(32 if mode=='editor' else 0)
            origin_x=float(row[9]);width=1280*scale
            picture=Image.open(state['screenshot']['path'])
            if picture.info.get('icc_profile'):
                picture=ImageCms.profileToProfile(picture,ImageCms.ImageCmsProfile(BytesIO(picture.info['icc_profile'])),ImageCms.createProfile('sRGB'),outputMode='RGB')
            a=np.array(picture.convert('RGB'));x=round(origin_x+width/2)
            rgb=np.array([255,0,255] if kind=='image' else [243,244,246])
            top=round(origin_y+48*scale);bottom=min(len(a),round(origin_y+672*scale))
            matching=np.flatnonzero(np.max(np.abs(a[top:bottom,x].astype(int)-rgb),axis=1)<=2)
            assert len(matching),(heading,kind)
            actual=top+int(matching[0]);expected=origin_y+base_y*scale
            records.append(dict(heading=heading,mode=mode,kind=kind,expected_top=expected,actual_top=actual))
            assert abs(actual-expected)<=1.1,records[-1]
            print('PASS',records[-1],flush=True)
            if mode=='presentation':gui.key('Escape')
    assert gui.fixture.read_bytes()==gui.original
finally:
    (gui.out/'following-layout-observations.json').write_text(json.dumps(records,indent=2)+'\n')
    gui.stop()
