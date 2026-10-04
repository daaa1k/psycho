import argparse,json,subprocess,time,unicodedata
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-columns-workflow')
source='''presentation {
 metadata { title "Columns workflow" }
 slide id="one" {
  heading "Columns workflow"
  text "Root payload"
  columns { column width=50 { text "Left one"; text "Left two"; }; column width=50 { bullets { item "Right first"; item "Right second"; }; }; }
  columns { column width=50 { text "Other left"; }; column width=50 { text "Other one"; text "Other two"; }; }
 }
}
'''
gui=Gui(parser.parse_args().output,source);records=[]
def select(prefix,font=28):
    gui.snapshot();x,y=gui.glyph_position(prefix,font);gui.click(x,y)
def operation(before,name):
    after=gui.save();assert after!=before,(name,'No movement')
    gui.snapshot(name)
    gui.press('Undo');assert gui.save()==before,(name,'One Undo')
    gui.snapshot(name+'-undo')
    gui.press('Redo');assert gui.save()==after,(name,'One Redo')
    gui.press('Undo');assert gui.save()==before
    records.append(dict(name=name,source=after.decode()))
    print('PASS',name,'one Undo/Redo and exact bytes',flush=True)
    return after
def drag(start,end,name):
    gui.act('get-app-state','--restore-window')
    process=subprocess.Popen(['xcrun','swift',str(root/'validation/mvp-acceptance/slow_drag.swift'),
        str(gui.app.pid),*map(str,(*start,*end)),'5'])
    try:
        time.sleep(3);gui.snapshot(name+'-held')
    finally:
        result=process.wait(timeout=30)
    assert result==0
    time.sleep(.4);gui.snapshot(name+'-dropped')
try:
    select('Root payload');gui.press('2列 3 · 左列へ移動')
    inside=operation(gui.original,'columns-root-to-column')
    assert inside.count(b'text "Root payload"')==1
    gui.press('Redo');assert gui.save()==inside
    select('Root payload')
    gui.press('Slide 直下へ移動')
    outside=operation(inside,'columns-column-to-root')
    assert outside.rindex(b'text "Root payload"')>outside.rindex(b'columns')
    gui.press('Undo');assert gui.save()==gui.original
    select('Left one');gui.press('列内 Element ↓')
    reordered=operation(gui.original,'columns-nested-reorder')
    assert reordered.index(b'text "Left two"')<reordered.index(b'text "Left one"')
    gui.snapshot();start=gui.glyph_position('Left one',28);end=gui.glyph_position('Other left',28)
    drag(start,(end[0],end[1]-8),'columns-between-blocks')
    moved=operation(gui.original,'columns-between-blocks')
    assert moved.count(b'text "Left one"')==1 and moved.index(b'text "Left one"')>moved.index(b'columns',moved.index(b'columns')+1)
    gui.snapshot();_,start_y=gui.glyph_position('Left one',28);end=gui.glyph_position('Other left',28)
    columns_gutter_x=574
    drag((columns_gutter_x,start_y),(end[0],end[1]-8),'columns-invalid-nesting')
    assert 'columnscannotbenested' in gui.text('columns-invalid-result')
    assert gui.save()==gui.original
    gui.press('Redo');assert gui.save()==moved,'Rejected drop must not clear Redo'
    gui.press('Undo');assert gui.save()==gui.original
    select('Left one');gui.press('2列の内容へ戻る');gui.press('Element を削除')
    deleted=operation(gui.original,'columns-delete-subtree')
    assert deleted.count(b'columns')==1 and b'Right first' not in deleted and b'Left one' not in deleted
    assert '左列50%/右列50%' in unicodedata.normalize('NFKC',gui.text('columns-restored-selection'))
    assert gui.save()==gui.original
    saved=gui.original
    for kind,payload,font,target in [('heading','Columns workflow',48,'2列 3 · 左列へ移動'),
                                      ('text','Root payload',28,'2列 2 · 左列へ移動')]:
        for location,label,depth in [('column',target,4),('root','Slide 直下へ移動',2)]:
            name=f'columns-{kind}-persist-{location}'
            select(payload,font);gui.press(label)
            saved=operation(saved,name)
            node=f'{kind} "{payload}"'.encode()
            assert saved.count(node)==1,(name,'Payload must occur once')
            preceding=saved[:saved.index(node)]
            assert preceding.count(b'{')-preceding.count(b'}')==depth,(name,'KDL parent')
            gui.press('Redo');assert gui.save()==saved
            for phase in ['saved','restarted']:
                if phase=='restarted':gui.stop();gui.start()
                assert gui.fixture.read_bytes()==saved,(name,phase,'Persisted KDL')
                assert ''.join(payload.split()) in gui.text(name+'-'+phase),(name,phase,'Visible payload')
                _,payload_y=gui.glyph_position(payload,font)
                _,other_y=gui.glyph_position('Other left',28)
                assert (payload_y<other_y)==(location=='column'),(name,phase,'Canvas placement')
                assert gui.save()==saved,(name,phase,'No-op save')
            print('PASS',name,'saved KDL and canvas survive restart',flush=True)
    select('Left one');gui.press('2列の内容へ戻る');gui.press('左列幅を数値入力')
    gui.replace('0');gui.press('保存')
    assert gui.fixture.read_bytes()==saved,'Invalid width must block save'
    assert gui.state()['text']=='0','Invalid width must remain editable'
    reason=gui.text('columns-invalid-width')
    assert '列幅は1' in reason and '99の整数' in reason,'Invalid width reason must be visible'
    gui.replace('50')
    assert gui.save()==saved,'Repair must leave the source unchanged'
    print('PASS invalid numeric width blocks save until repaired',flush=True)
    print('PASS root/column moves, nested reorder, inter-block drag, invalid nesting, Columns subtree deletion, selection restoration and heading/body persistence',flush=True)
except Exception:
    gui.snapshot('columns-failure');raise
finally:
    (gui.out/'columns-workflow-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
