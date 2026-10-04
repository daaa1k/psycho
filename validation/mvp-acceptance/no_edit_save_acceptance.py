"""Unchanged fields keep raw quoting, empty lists and absent optional fields."""
import argparse
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-no-edit-save')
source='presentation { metadata { title #"Raw title"# }; slide { heading #"Raw heading"#; bullets {}; image "missing.png"; code #"body"#; }; }\r\n'
gui=Gui(parser.parse_args().output,source)
try:
    assert gui.save()==gui.original,'No-edit Save must preserve all raw quotes and CRLF bytes'
    gui.edit('Raw heading',48);gui.key('Escape')
    assert gui.save()==gui.original
    # Select the missing image's placeholder, then leave its absent Caption
    # unchanged. This must not insert an empty Caption into KDL.
    gui.snapshot();gui.click(580,470);gui.press('Caption');gui.key('Escape')
    assert gui.save()==gui.original,'An absent empty Caption must remain absent'
    gui.edit('body',22);gui.key('Escape');gui.press('コード言語');gui.key('Escape')
    assert gui.save()==gui.original,'An absent empty language must remain absent'
    gui.hotkey('CmdOrCtrl+Z');assert gui.save()==gui.original
    gui.snapshot('no-edit-save-preserved')
    print('PASS no-edit Save, raw quotes, CRLF, unchanged Caption/language and no added Undo',flush=True)
except Exception:
    gui.snapshot('no-edit-save-failure');raise
finally:gui.stop()
