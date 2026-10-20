import argparse
import json
import re
import shutil
from pathlib import Path

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
gui = Gui(parser.parse_args().output,
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
                        editor=gui.state(), tree=state['snapshot']['treeText'],
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
    gui.press('＋ Slide')
    for label, value in [('見出し', 'Authored heading'), ('本文', 'Authored body'),
                         ('箇条書き', 'Authored bullet'), ('コード', 'println!("Authored code");')]:
        add(label)
        gui.replace(value)
        gui.key('Escape')
    add('画像')
    gui.replace('assets/image.png')
    gui.press('適用', scroll=True)
    gui.press('Caption', scroll=True)
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
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(gui.out), '--no-screenshot')
    gui.key('Return')
    gui.button('Save')
    saved = saved_file.read_bytes()
    assert len(re.findall(rb'\bslide\b', saved)) == 2, saved
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
    undone = gui.save()
    assert b'Authored right column' not in undone and undone != saved, undone
    gui.press('Redo')
    assert gui.save() == saved
    observe('exact-redo-after-save-as')
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
