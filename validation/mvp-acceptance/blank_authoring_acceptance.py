import argparse
import json
import os
import re
import shutil
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
out = parser.parse_args().output.resolve()
os.environ['PSYCHO_EDITOR_STATE'] = str(out / 'editor-state.json')
gui = Gui(out,
          'presentation { metadata { title "Unused placeholder" }; }\n',
          name='unused-placeholder', untitled=True)
records = []
saved_file = gui.out / 'presentation.kdl'


def ax(label):
    state = gui.act('get-app-state')
    tree = state['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) (?:button|radio button) (?:\([^)]*\) )?' +
                      re.escape(label) + r'(?: ✓)?(?:, .*)?$', tree, re.M)
    assert match, (label, tree)
    gui.act('click', '--element-index', match[1], '--no-screenshot')


def add(label, parent=None):
    ax('挿入')
    if parent:
        ax(parent)
    ax(label)


def observe(step):
    state, _ = gui.snapshot(step)
    records.append(dict(step=step, window=state['snapshot']['window'],
                        editor=json.loads((gui.out / 'editor-state.json').read_text()),
                        input=gui.state(), tree=state['snapshot']['treeText'],
                        kdl=saved_file.read_text() if saved_file.exists() else None))


try:
    assets = gui.out / 'assets'
    assets.mkdir()
    shutil.copy(root / 'assets/build-time.png', assets / 'image.png')
    initial = gui.act('get-app-state')['snapshot']['treeText']
    assert 'スライド 6' in initial or 'CI' in gui.text(), initial
    for _ in range(6):
        gui.press('− Slide')
    empty = gui.act('get-app-state')['snapshot']['treeText']
    assert not re.search(r'text (?:\([^)]*\) )?スライド \d+', empty), empty
    observe('zero-slides')
    gui.press('Presentation title')
    gui.focus_input('Presentation のタイトル')
    gui.replace('Authored presentation')
    gui.press('適用')
    gui.press('＋ Slide')
    for label, value in [('見出し', 'Authored heading'), ('本文', 'Authored body'),
                         ('箇条書き', 'Authored bullet'), ('コード', 'println!("Authored code");')]:
        add(label)
        gui.replace(value)
        gui.key('Escape')
    gui.press('＋ Slide')
    add('画像')
    gui.replace(os.path.relpath(assets / 'image.png', root / 'examples'))
    gui.press('適用', scroll=True)
    gui.press('Caption', scroll=True)
    gui.focus_input('画像のCaption')
    gui.replace('Authored image caption')
    gui.press('適用', scroll=True)
    gui.key('Escape')
    observe('authored-content')
    gui.press('＋ Slide')
    add('2列')
    gui.key('Escape')
    for side, value in [('左列', 'Authored left column'), ('右列', 'Authored right column')]:
        add('本文', f'2列 1 の{side}')
        gui.replace(value)
        gui.key('Escape')
    gui.press('保存')
    gui.button('退避先を選ぶ')
    gui.hotkey('CmdOrCtrl+Shift+G')
    panel = gui.act('get-app-state')['snapshot']['treeText']
    assert re.search(r'The focused UI element is \d+ text field \(settable\), Value:', panel), panel
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(gui.out), '--no-screenshot')
    gui.key('Return')
    gui.button('Save')
    saved = saved_file.read_bytes()
    assert len(re.findall(rb'\bslide\b', saved)) == 3, saved
    assert b'title "Authored presentation"' in saved
    for kind in ('heading', 'text', 'bullets', 'code', 'image', 'columns'):
        assert re.search(rb'\b' + kind.encode() + rb'\b', saved), (kind, saved)
    for value in ('Authored heading', 'Authored body', 'Authored bullet', 'Authored code',
                  'Authored image caption', 'Authored left column', 'Authored right column'):
        assert value.encode() in saved, (value, saved)
    assert b'image "assets/image.png"' in saved
    assert b'CI ' not in saved and b'Unused placeholder' not in saved
    assert re.search(rb'column width=50\s*\{\s*text "Authored left column"', saved), saved
    assert re.search(rb'column width=50\s*\{\s*text "Authored right column"', saved), saved
    observe('saved-all-six-kinds')
    gui.fixture = saved_file
    gui.press('Undo')
    assert gui.save() == saved, 'Save As clears document history'
    gui.press('Redo')
    assert gui.save() == saved
    observe('history-cleared-after-save-as')
    gui.stop()
    gui.untitled = False
    gui.start()
    assert gui.fixture.read_bytes() == saved
    gui.select_slide(1)
    gui.edit('Authored heading', 48)
    assert gui.state()['text'] == 'Authored heading' and gui.state()['undo'] == 0
    gui.key('Escape')
    gui.hotkey('CmdOrCtrl+Z')
    assert gui.save() == saved
    observe('restarted-history-cleared')
    gui.press('最初から発表')
    state, rows = gui.snapshot('presenting-authored-content')
    assert state['snapshot']['window']['width'] != 1280
    text = ''.join(''.join(row['text'].split()) for row in rows)
    assert 'Authoredheading' in text and 'Authoredbody' in text, text
    assert saved_file.read_bytes() == saved
    gui.key('Right')
    state, rows = gui.snapshot('presenting-authored-image')
    text = ''.join(''.join(row['text'].split()) for row in rows)
    assert 'Authoredimagecaption' in text, text
    assert saved_file.read_bytes() == saved
    gui.key('Right')
    state, rows = gui.snapshot('presenting-authored-columns')
    text = ''.join(''.join(row['text'].split()) for row in rows)
    assert 'Authoredleftcolumn' in text and 'Authoredrightcolumn' in text, text
    assert saved_file.read_bytes() == saved
    gui.key('Escape')
    state, _ = gui.snapshot('returned-to-authoring')
    assert state['snapshot']['window']['width'] == 1280
    assert gui.save() == saved
    observe('returned-content-retained')
    print('PASS blank authoring, all six kinds, Save As, history, restart and presentation', flush=True)
except Exception:
    gui.snapshot('blank-authoring-failure')
    raise
finally:
    (gui.out / 'blank-authoring-observations.json').write_text(
        json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
