import argparse,json,re,shutil,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-sample-workflow')
source=(root/'examples/build-time.kdl').read_text().replace('../assets/build-time.png','assets/build-time.png')
gui=Gui(parser.parse_args().output,source);records=[]
assets=gui.out/'assets';assets.mkdir()
for name in ['build-time.png','updated.png']:shutil.copy(root/'assets/build-time.png',assets/name)
def compact(text):return ''.join(text.split())
def select_slide(number):
    gui.select_slide(number)
def history_check(before,name):
    saved=gui.save();assert saved!=before,(name,saved)
    gui.snapshot(name)
    gui.press('Undo');assert gui.save()==before,(name,'Undo')
    gui.press('Redo');assert gui.save()==saved,(name,'Redo')
    records.append(dict(name=name,source=saved.decode()))
    print('PASS',name,'Save / exact Undo / exact Redo',flush=True)
    return saved
def field(prefix,font,new):
    gui.edit(prefix,font);gui.replace(new);gui.key('Escape')
try:
    gui.stop();gui.start()
    gui.press('Presentation title')
    _,rows=gui.snapshot();row=next(row for row in rows if row['x']>1000 and compact(row['text'])=='ビルド時間を38%短縮した話')
    gui.click(row['x'],row['y']);gui.replace('ビルド時間を40%短縮した話');gui.press('適用')
    saved=history_check(gui.original,'sample-title')
    field('CI のビルドが遅い',48,'CI のビルドを改善')
    saved=history_check(saved,'sample-heading')
    field('変更のたびに',28,'変更のたびの待ち時間を減らし、レビューを進める。')
    saved=history_check(saved,'sample-body')
    select_slide(2)
    field('let key',22,'let\tkey = hash(lockfile, toolchain, target);\nassert!(cache_hit);')
    saved=history_check(saved,'sample-code')
    select_slide(3)
    x,y=gui.glyph_position('依存関係',48);gui.click(x,y)
    gui.press('2列の内容へ戻る');gui.press('左列幅を数値入力');gui.replace('50')
    gui.press('幅を適用',scroll=True,aliases=('福を適用',))
    saved=history_check(saved,'sample-column-width')
    assert b'column width=50' in saved and b'column width=45' not in saved and b'column width=55' not in saved
    select_slide(4)
    gui.edit('キャッシュ導入前後',20);gui.press('画像パス');gui.replace('assets/updated.png');gui.press('適用')
    saved=history_check(saved,'sample-image-path')
    gui.press('Caption');gui.replace('キャッシュ導入前後の CI 実行時間を比較');gui.key('Escape')
    saved=history_check(saved,'sample-caption')
    select_slide(1)
    gui.edit('変更のたびの',28);gui.key('Escape');gui.press('Element ↑')
    saved=history_check(saved,'sample-structure')
    expected=['CI のビルドを改善','キャッシュの境界を見直す','依存と成果物を分ける',
        '計測方法','ビルド時間を38%短縮','待ち時間を減らし、変更を速く届ける']
    gui.press('最初から発表')
    for i,heading in enumerate(expected):
        if i:gui.key('Right')
        state,rows=gui.snapshot(f'sample-presentation-{i+1}')
        assert state['screenshot']['width']==1920 and compact(heading) in compact(''.join(row['text'] for row in rows))
        assert gui.fixture.read_bytes()==saved
    gui.key('Escape');time.sleep(1)
    state,rows=gui.snapshot('sample-return')
    assert state['screenshot']['width']==1280 and compact(expected[-1]) in compact(''.join(row['text'] for row in rows))
    gui.stop();gui.start()
    for i,heading in enumerate(expected):
        select_slide(i+1);gui.edit(heading,48)
        assert gui.state()['text']==heading and gui.state()['undo']==0
        gui.key('Escape')
    gui.hotkey('CmdOrCtrl+Z');gui.press('保存')
    assert gui.fixture.read_bytes()==saved,'Reload must clear history and retain the saved edits'
    gui.snapshot('sample-reloaded')
    print('PASS six-slide sample edits, structure, width, image reference, exact Undo/Redo, reload and full presentation',flush=True)
except Exception:
    gui.snapshot('sample-failure')
    raise
finally:
    (gui.out/'sample-workflow-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
