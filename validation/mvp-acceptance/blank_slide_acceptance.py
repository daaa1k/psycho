"""Add every Element to a blank Slide and exercise populated Slide history."""
import argparse,json,re,shutil
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-blank-slide')
out=parser.parse_args().output
source='presentation {\n metadata { title "Blank additions" }\n slide id="blank" {}\n}\n'
gui=Gui(out,source);records=[]
asset=gui.out/'assets/image.png';asset.parent.mkdir();shutil.copy(root/'assets/build-time.png',asset)
asset_original=asset.read_bytes()
def checkpoint(name):
    state,rows=gui.snapshot(name)
    records.append(dict(name=name,source=gui.fixture.read_text(),text=[r['text'] for r in rows]))
    print('PASS',name,flush=True)
def slides(data):return len(re.findall(rb'\bslide(?:\s+id="[^"]*")?\s*\{',data))
try:
    for kind,label in [('heading','見出しを追加'),('text','本文を追加'),('bullets','箇条書きを追加'),('code','コードを追加'),('image','画像参照を追加'),('columns','2列を追加')]:
        gui.press(label);gui.key('Escape');added=gui.save()
        assert re.search(rb'\b'+kind.encode()+rb'\b',added) and slides(added)==1,(kind,added)
        checkpoint(f'blank-add-{kind}')
        if kind=='image':
            gui.press('Element を削除');deleted=gui.save()
            assert b'image "' not in deleted and asset.read_bytes()==asset_original
            gui.press('Undo');assert gui.save()==added,'One Undo must restore the image Element without modifying its Asset'
            checkpoint('blank-image-delete-undo')
        gui.press('Undo');assert gui.save()==gui.original,(kind,gui.fixture.read_bytes())
    gui.press('＋ Slide');assert slides(gui.save())==2
    gui.press('見出しを追加');gui.replace('Second slide');gui.key('Escape')
    gui.press('本文を追加');gui.replace('Body retained with slide');gui.key('Escape')
    populated=gui.save();assert b'heading "Second slide"' in populated and b'text "Body retained with slide"' in populated
    checkpoint('slide-populated')
    gui.press('Slide ↑');up=gui.save()
    assert up.index(b'heading "Second slide"')<up.index(b'slide id="blank"')
    checkpoint('slide-move-up')
    gui.press('Slide ↓');down=gui.save()
    assert down.index(b'heading "Second slide"')>down.index(b'slide id="blank"')
    checkpoint('slide-move-down')
    gui.press('Undo');assert gui.save()==up
    gui.press('Undo');assert gui.save()==populated
    gui.press('− Slide');deleted=gui.save()
    assert slides(deleted)==1 and b'Second slide' not in deleted and b'Body retained with slide' not in deleted
    checkpoint('slide-delete-populated')
    gui.press('Undo');assert gui.save()==populated,'One Undo must restore the complete Slide and all source bytes'
    checkpoint('slide-delete-undo')
    assert asset.read_bytes()==asset_original
    print('PASS all six Element additions, image deletion without Asset deletion, Slide add/move/delete and exact Undo',flush=True)
finally:
    (gui.out/'blank-slide-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
