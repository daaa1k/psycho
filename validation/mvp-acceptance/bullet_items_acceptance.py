"""Bullet item addition, movement and deletion at root and inside Columns."""
import argparse,json
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-bullet-items')
parser.add_argument('--nested',action='store_true');args=parser.parse_args()
bullets='bullets { item "First item"; item "Second item"; }'
element='columns { column width=50 { '+bullets+'; }; column width=50 { text "Sibling untouched"; }; }' if args.nested else bullets
gui=Gui(args.output,'presentation { metadata { title "Bullet items" }; slide { heading "Bullet items"; '+element+'; }; }\n')
records=[]
def save_history(before,name):
    after=gui.save();assert after!=before,(name,'No edit')
    gui.press('Undo');assert gui.save()==before,(name,'One Undo')
    gui.press('Redo');assert gui.save()==after,(name,'One Redo')
    gui.snapshot(name);records.append(dict(name=name,source=after.decode()))
    print('PASS',name,flush=True);return after
try:
    gui.snapshot();x,y=gui.glyph_position('First item',28);gui.click(x,y)
    gui.press('項目を追加');gui.replace('Third item');gui.key('Escape')
    added=gui.save();assert b'item "Third item"' in added, 'Adding an item must not commit the old whole-list draft over it'
    # The structural addition and its text are distinct editing operations.
    gui.press('Undo');empty=gui.save();assert b'item ""' in empty and b'Third item' not in empty
    gui.press('Undo');assert gui.save()==gui.original
    gui.press('Redo');assert gui.save()==empty
    gui.press('Redo');assert gui.save()==added
    gui.snapshot();x,y=gui.glyph_position('Third item',28);gui.click(x,y)
    gui.press('項目 3');gui.key('Escape');gui.press('項目を上へ')
    moved=save_history(added,'bullet-item-move')
    assert moved.index(b'Third item')<moved.index(b'Second item')
    gui.press('項目を削除');deleted=save_history(moved,'bullet-item-delete')
    assert b'Third item' not in deleted and b'First item' in deleted and b'Second item' in deleted
    if args.nested:assert b'text "Sibling untouched"' in deleted
    print('PASS bullet item add, text, move, delete and exact history; nested='+str(args.nested),flush=True)
except Exception:
    gui.snapshot('bullet-items-failure');raise
finally:
    (gui.out/'bullet-items-observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    gui.stop()
