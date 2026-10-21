import argparse,json,re,shutil,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-sample-workflow')
source=(root/'examples/build-time.kdl').read_text().replace('../assets/build-time.png','assets/build-time.png')
gui=Gui(parser.parse_args().output,source);records=[]
assets=gui.out/'assets';assets.mkdir()
(gui.out/'original.kdl').write_bytes(gui.original)
for name in ['build-time.png','updated.png']:shutil.copy(root/'assets/build-time.png',assets/name)
def compact(text):return ''.join(text.split())
def select_slide(number):
    gui.select_slide(number)
def history_check(before,name):
    saved=gui.save();assert saved!=before,(name,saved)
    state,_=gui.snapshot(name)
    gui.press('Undo');assert gui.save()==before,(name,'Undo')
    gui.press('Redo');assert gui.save()==saved,(name,'Redo')
    records.append(dict(step=name,window=state['snapshot']['window'],editor=gui.state(),kdl=saved.decode()))
    print('PASS',name,'Save / exact Undo / exact Redo',flush=True)
    return saved
def field(prefix,font,new):
    gui.edit(prefix,font);gui.replace(new);gui.key('Escape')
try:
    gui.stop();gui.start()
    gui.press('Presentation title')
    gui.focus_input('Presentation のタイトル')
    gui.replace('ビルド時間を40%短縮した話');gui.press('適用')
    saved=history_check(gui.original,'sample-title')
    assert 'title "ビルド時間を40%短縮した話"' in saved.decode()
    field('CI のビルドが遅い',48,'CI のビルドを改善')
    saved=history_check(saved,'sample-heading')
    assert 'heading "CI のビルドを改善"' in saved.decode()
    field('変更のたびに',28,'変更のたびの待ち時間を減らし、レビューを進める。')
    saved=history_check(saved,'sample-body')
    assert 'text "変更のたびの待ち時間を減らし、レビューを進める。"' in saved.decode()
    field('平均 8 分 12 秒',28,'平均 5 分 06 秒')
    saved=history_check(saved,'sample-bullet-item')
    assert 'item "平均 5 分 06 秒"' in saved.decode()
    select_slide(2)
    field('let key',22,'let\tkey = hash(lockfile, toolchain, target);\nassert!(cache_hit);')
    saved=history_check(saved,'sample-code')
    assert b'let\\tkey = hash(lockfile, toolchain, target);\\nassert!(cache_hit);' in saved
    select_slide(3)
    gui.outline_select('Element 2 · 2列')
    gui.press('左列幅を数値入力',scroll=True);gui.focus_input('左列幅、1〜99の整数');gui.replace('50')
    gui.press('幅を適用',scroll=True,aliases=('福を適用',))
    saved=history_check(saved,'sample-column-width')
    assert b'column width=50' in saved and b'column width=45' not in saved and b'column width=55' not in saved
    select_slide(4)
    gui.edit('キャッシュ導入前後',20);gui.press('画像パス');gui.focus_input('画像パス、文書からの相対パス');gui.replace('assets/updated.png');gui.press('適用',scroll=True)
    saved=history_check(saved,'sample-image-path')
    assert b'image "assets/updated.png"' in saved
    gui.press('Caption');gui.focus_input('画像のCaption');gui.replace('キャッシュ導入前後の CI 実行時間を比較');gui.press('適用',scroll=True)
    saved=history_check(saved,'sample-caption')
    assert 'caption "キャッシュ導入前後の CI 実行時間を比較"' in saved.decode()
    select_slide(1)
    gui.edit('変更のたびの',28);gui.key('Escape');gui.press('Element ↑')
    saved=history_check(saved,'sample-structure')
    assert saved.index('text "変更のたびの待ち時間を減らし、レビューを進める。"'.encode()) < saved.index('heading "CI のビルドを改善"'.encode())
    gui.press('Slide ↓')
    saved=history_check(saved,'sample-slide-move-later')
    assert saved.index(b'id="approach"') < saved.index(b'id="problem"')
    gui.menu('表示','Overview')
    state,_=gui.snapshot('sample-overview')
    assert 'Overview' in state['snapshot']['treeText']
    assert gui.save()==saved
    gui.menu('表示','編集')
    assert gui.save()==saved
    gui.press('Slide ↑')
    saved=history_check(saved,'sample-slide-move-earlier')
    expected=['CI のビルドを改善','キャッシュの境界を見直す','依存と成果物を分ける',
        '計測方法','ビルド時間を38%短縮','待ち時間を減らし、変更を速く届ける']
    gui.stop();gui.start()
    for i,heading in enumerate(expected):
        select_slide(i+1);gui.edit(heading,48)
        assert gui.state()['text']==heading and gui.state()['undo']==0
        gui.key('Escape')
    gui.hotkey('CmdOrCtrl+Z');gui.press('保存')
    assert gui.fixture.read_bytes()==saved,'Reload must clear history and retain the saved edits'
    gui.snapshot('sample-reloaded-before-presentation')
    select_slide(3)
    gui.press('現在から発表')
    state,rows=gui.snapshot('sample-presentation-current')
    assert state['snapshot']['window']['width']!=1280
    assert compact(expected[2]) in compact(''.join(row['text'] for row in rows))
    assert gui.fixture.read_bytes()==saved
    gui.key('Escape')
    gui.press('最初から発表')
    for i,heading in enumerate(expected):
        if i:gui.key('Right')
        state,rows=gui.snapshot(f'sample-presentation-{i+1}')
        assert state['snapshot']['window']['width']!=1280 and compact(heading) in compact(''.join(row['text'] for row in rows))
        assert gui.fixture.read_bytes()==saved
    gui.key('Left')
    _,rows=gui.snapshot('sample-presentation-previous')
    assert compact(expected[-2]) in compact(''.join(row['text'] for row in rows))
    gui.key('Right')
    gui.key('Escape');time.sleep(1)
    state,rows=gui.snapshot('sample-return')
    assert state['snapshot']['window']['width']==1280 and compact(expected[-1]) in compact(''.join(row['text'] for row in rows))
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
