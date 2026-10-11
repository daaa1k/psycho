"""Native Edit menu clicks must route to field/document history and clipboard scope.

Run against a fresh --output under target/. Never substitutes undo/redo hotkeys.
The AX helper observes and presses native menu items only in the owned process.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

from gui_session import Gui, root


def nodes(node):
    yield node
    for child in node.get('children', []):
        yield from nodes(child)


class NativeMenu:
    def __init__(self, gui):
        self.gui = gui
        self.helper = gui.out / 'edit-menu-state'
        self.records = []
        subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('edit_menu_state.swift')),
                        '-o', str(self.helper)], check=True)

    def observe(self, name):
        metadata = json.loads(subprocess.check_output([str(self.helper), str(self.gui.app.pid)], text=True))
        state = self.gui.act('get-app-state')
        shutil.copy(state['screenshot']['path'], self.gui.out / f'{name}.png')
        record = dict(name=name, ax=metadata, tree=state['snapshot']['treeText'],
                      input=self.gui.state(), savedSha256=hashlib.sha256(self.gui.fixture.read_bytes()).hexdigest())
        self.records.append(record)
        (self.gui.out / 'edit-menu-observations.json').write_text(json.dumps(self.records, ensure_ascii=False, indent=2))
        return metadata

    def click(self, title):
        self.gui.act('get-app-state', '--restore-window')
        subprocess.run([str(self.helper), str(self.gui.app.pid), title], check=True, stdout=subprocess.DEVNULL)

    def check(self, name, undo, redo, clipboard=False):
        self.click('編集')
        metadata = self.observe(name)
        menu = [node for node in nodes(metadata['menuBar']) if node.get('title') == '編集']
        assert len(menu) == 1, metadata
        items = list(nodes(menu[0]))
        for prefix, expectation in [('元に戻す', undo), ('やり直す', redo)]:
            item = [node for node in items if node.get('title', '').startswith(prefix)]
            assert len(item) == 1, (prefix, items)
            assert item[0].get('enabled') is expectation[1], (name, item[0], expectation)
            assert item[0]['title'] == expectation[0], (name, item[0], expectation)
        for title in ['切り取り', 'コピー', '貼り付け']:
            item = next(node for node in items if node.get('title') == title)
            assert item.get('enabled') is clipboard, (name, title, item)
        self.gui.key('Escape')

    def action(self, title, name):
        self.click('編集')
        metadata = self.observe(name + '-before')
        item = next(node for node in nodes(metadata['menuBar']) if node.get('title') == title)
        assert item.get('enabled') is True, item
        self.click(title)
        self.observe(name + '-after')


parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'target/mvp-edit-menu')
args = parser.parse_args()
source = 'presentation { metadata { title "Native menu" }; slide id="one" { heading #"original"#; }; }\n'
gui = Gui(args.output, source)
menu = None
try:
    menu = NativeMenu(gui)
    menu.check('empty-document', ('元に戻す', False), ('やり直す', False))
    gui.edit('original', 48)
    gui.hotkey('CmdOrCtrl+A')
    menu.check('empty-field-selected', ('元に戻す: 文字編集', False), ('やり直す: 文字編集', False), True)
    subprocess.run(['pbcopy'], input=b'copy-sentinel', check=True)
    menu.action('コピー', 'copy')
    assert subprocess.check_output(['pbpaste']) == b'original'
    assert gui.state()['text'] == 'original' and gui.fixture.read_bytes() == gui.original
    menu.action('切り取り', 'cut')
    assert gui.state()['text'] == '' and gui.fixture.read_bytes() == gui.original
    menu.action('元に戻す: 文字編集', 'cut-undo')
    assert gui.state()['text'] == 'original' and gui.fixture.read_bytes() == gui.original
    gui.hotkey('CmdOrCtrl+A')
    subprocess.run(['pbcopy'], input=b'changed', check=True)
    menu.action('貼り付け', 'paste')
    assert gui.state()['text'] == 'changed' and gui.fixture.read_bytes() == gui.original
    gui.hotkey('CmdOrCtrl+A')
    menu.check('field-changed', ('元に戻す: 文字編集', True), ('やり直す: 文字編集', False), True)
    menu.action('元に戻す: 文字編集', 'field-undo')
    assert gui.state()['text'] == 'original' and gui.fixture.read_bytes() == gui.original
    menu.action('やり直す: 文字編集', 'field-redo')
    assert gui.state()['text'] == 'changed' and gui.fixture.read_bytes() == gui.original
    saved = gui.save()
    assert b'heading "changed"' in saved and saved != gui.original
    gui.click(800, 650)
    metadata = menu.observe('document-changed')
    undo = next(node for node in nodes(metadata['menuBar']) if node.get('title', '').startswith('元に戻す'))
    assert undo['title'].startswith('元に戻す: ') and undo['title'] != '元に戻す: 文字編集'
    label = undo['title'].split(': ', 1)[1]
    menu.check('document-menu', (undo['title'], True), ('やり直す', False))
    menu.action(undo['title'], 'document-undo')
    assert gui.fixture.read_bytes() == saved, 'Undo must not save implicitly'
    assert gui.save() == gui.original, 'Native document Undo must restore every original byte'
    menu.check('document-undone', ('元に戻す', False), ('やり直す: ' + label, True))
    menu.action('やり直す: ' + label, 'document-redo')
    assert gui.fixture.read_bytes() == gui.original
    assert gui.save() == saved, 'Native document Redo must restore saved edited bytes'
    menu.observe('complete')
    print('PASS native Edit menu history, dynamic labels, disabled states and clipboard scope', flush=True)
except Exception:
    gui.snapshot('edit-menu-failure')
    raise
finally:
    gui.stop()
