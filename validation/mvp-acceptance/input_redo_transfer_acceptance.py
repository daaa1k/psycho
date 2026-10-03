"""Ending input or saving must retain its undone valid units as global Redo."""
import argparse,json
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-input-redo-transfer')
parser.add_argument('--end',choices=['save','escape','slide','background'],default='save');args=parser.parse_args()
source='presentation { metadata { title "Redo transfer" }; slide id="one" { heading #"original"#; }; slide id="two" { heading "Other slide"; }; }\n'
gui=Gui(args.output,source)
try:
    gui.edit('original',48);gui.replace('first');gui.replace('second')
    gui.hotkey('CmdOrCtrl+Z')
    assert gui.state()['text']=='first' and gui.state()['undo']==1 and gui.state()['redo']==1
    if args.end=='background':
        gui.hotkey('CmdOrCtrl+Z');assert gui.state()['text']=='original'
        gui.click(800,650);assert not gui.state()['focused']
        gui.hotkey('CmdOrCtrl+Shift+Z')
        assert b'heading "first"' in gui.save(), 'Focus loss must transfer Redo even when every input unit is undone'
    if args.end=='escape':gui.key('Escape')
    elif args.end=='slide':gui.select_slide(2)
    first=gui.save();assert b'heading "first"' in first
    gui.hotkey('CmdOrCtrl+Shift+Z')
    assert gui.fixture.read_bytes()==first
    second=gui.save();assert b'heading "second"' in second, 'Save/edit completion must preserve Redo for undone input units'
    gui.snapshot('redo-after-'+args.end)
    if args.end=='slide':
        _,rows=gui.snapshot('redo-target-slide')
        assert any(166<row['x']<995 and row['text'].strip()=='second' for row in rows), 'Redo must display the changed Slide'
    gui.press('Undo');assert gui.save()==first
    gui.press('Undo');assert gui.save()==gui.original, 'History transfer must restore the original raw quote and all bytes'
    gui.press('Redo');assert gui.save()==first
    gui.press('Redo');assert gui.save()==second
    gui.snapshot('redo-history-restored')
    print('PASS input Redo transfer after '+args.end,flush=True)
except Exception:
    gui.snapshot('redo-transfer-failure');raise
finally:gui.stop()
