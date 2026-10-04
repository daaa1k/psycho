"""Check actual image/code rectangles following empty/tall columns and captions."""
import argparse,csv,json,re,time
from io import BytesIO
from pathlib import Path
import numpy as np
from PIL import Image,ImageCms
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-following-layout')
parser.add_argument('--case',type=int,action='append');args=parser.parse_args()
source='''presentation {
 metadata { title "Following layout" }
 slide { heading "Empty columns"; columns { column width=50 {}; column width=50 {}; }; image "marker.png"; }
 slide { heading "Tall columns"; columns { column width=50 { text "First\\nSecond\\nThird"; }; column width=50 { text "Short"; }; }; image "marker.png"; }
 slide { heading "No caption"; image "marker.png"; code "marker();"; }
 slide { heading "Empty caption"; image "marker.png" { caption ""; }; code "marker();"; }
 slide { heading "Nested tall"; columns { column width=50 { text "First\\nSecond\\nThird"; image "marker.png"; }; column width=50 { text "Short"; }; }; }
 slide { heading "Nested caption"; columns { column width=50 { image "marker.png" { caption ""; }; code "marker();"; }; column width=50 {}; }; }
}
'''
gui=Gui(args.output,source);records=[]
try:
    Image.new('RGB',(256,128),(255,0,255)).save(gui.out/'marker.png')
    gui.stop();gui.start()
    for i,(heading,kind,base_y) in enumerate([
        ('Empty columns','image',156),('Tall columns','image',282),
        ('No caption','code',476),('Empty caption','code',476),
        ('Nested tall','nested-image',301),('Nested caption','nested-code',476)]):
        if args.case and i+1 not in args.case:continue
        if i:
            gui.select_slide(i+1)
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
            a=np.array(picture.convert('RGB'));x=round(origin_x+(346*scale if kind.startswith('nested-') else width/2))
            is_image=kind.endswith('image')
            rgb=np.array([255,0,255] if is_image else [243,244,246])
            top=round(origin_y+48*scale);bottom=min(len(a),round(origin_y+672*scale))
            expected=origin_y+base_y*scale
            if not is_image:top=max(top,round(expected-4*scale))
            pixels=a[top:bottom,x].astype(int)
            # Include the antialiased leading edge, rather than the first
            # completely opaque scanline a further pixel inside the image.
            mask=(pixels[:,0]-pixels[:,1]>80)&(pixels[:,2]-pixels[:,1]>80) if is_image else (
                (np.max(np.abs(pixels-rgb),axis=1)<=10)&(pixels[:,0]<254))
            matching=np.flatnonzero(mask)
            assert len(matching),(heading,kind)
            actual=top+int(matching[0])
            records.append(dict(heading=heading,mode=mode,kind=kind,expected_top=expected,actual_top=actual))
            assert abs(actual-expected)<=1.1,records[-1]
            if is_image:
                magenta=(a[:,:,0].astype(int)-a[:,:,1]>80)&(a[:,:,2].astype(int)-a[:,:,1]>80)
                yy,xx=np.nonzero(magenta)
                actual_size=[int(xx.max()-xx.min()+1),int(yy.max()-yy.min()+1)]
                base_width=564 if kind.startswith('nested-') else 640
                expected_size=[base_width*scale,base_width/2*scale]
                assert all(abs(actual_size[j]-expected_size[j])<=1.1 for j in [0,1]),(heading,actual_size,expected_size)
                records[-1].update(actual_image_size=actual_size,expected_image_size=expected_size)
            print('PASS',records[-1],flush=True)
            if mode=='presentation':
                gui.key('Escape');time.sleep(1)
                assert gui.snapshot()[0]['screenshot']['width']==1280
    assert gui.fixture.read_bytes()==gui.original
finally:
    (gui.out/'following-layout-observations.json').write_text(json.dumps(records,indent=2)+'\n')
    gui.stop()
