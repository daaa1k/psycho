import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--combined-only', action='store_true')
args = parser.parse_args()
out = args.output
source = 'presentation {\n metadata { title "Insertion" }\n slide id="blank" {}\n}\n'
gui = Gui(out, source)
records = []

def ax(label):
    state = gui.act('get-app-state')
    tree = state['snapshot']['treeText']
    match = re.search(r'^\s*(\d+) (?:button|radio button) (?:\([^)]*\) )?' + re.escape(label) + r'(?: ✓)?(?:, (?:Secondary Actions:|Value:).*)?$', tree, re.M)
    assert match, (label, tree)
    gui.act('click', '--element-index', match[1], '--no-screenshot')

def add(label, parent=None):
    ax('挿入')
    if parent:
        ax(parent)
    ax(label)

def checkpoint(name):
    state, rows = gui.snapshot(name)
    records.append({'name': name, 'tree': state['snapshot']['treeText'], 'source': gui.fixture.read_text()})
    print('PASS', name, flush=True)

try:
    asset = gui.out / 'assets/image.png'
    asset.parent.mkdir()
    shutil.copy(root / 'assets/build-time.png', asset)
    asset_original = asset.read_bytes()
    if not args.combined_only:
        for kind, label in [('heading', '見出し'), ('text', '本文'), ('bullets', '箇条書き'), ('code', 'コード'), ('image', '画像'), ('columns', '2列')]:
            add(label)
            if kind in ('heading', 'text', 'bullets', 'code'):
                assert gui.state()['focused'], (kind, gui.state())
            if kind == 'image':
                assert gui.state()['focused'] and gui.state()['text'] == 'assets/image.png', gui.state()
            gui.key('Escape')
            saved = gui.save()
            assert re.search(rb'\b' + kind.encode() + rb'\b', saved), saved
            checkpoint('root-' + kind)
            ax('元に戻す')
            undone = gui.save()
            assert undone == gui.original, (kind, undone)
            ax('やり直す')
            redone = gui.save()
            assert redone == saved
            records.append({'name': 'root-' + kind + '-history', 'added': saved.decode(), 'undone': undone.decode(), 'redone': redone.decode()})
            ax('元に戻す')
            assert gui.save() == gui.original
    for label, value in [('見出し', 'Title'), ('本文', 'Body'), ('箇条書き', 'Item'), ('コード', 'x')]:
        add(label)
        gui.replace(value)
        gui.key('Escape');gui.save()
    gui.menu('表示', 'Inspectorを隠す')
    add('画像')
    tree = gui.act('get-app-state')['snapshot']['treeText']
    assert '画像パス' in tree and gui.state()['focused'], tree
    gui.key('Escape')
    image_added = gui.save()
    ax('Element を削除')
    image_deleted = gui.save()
    assert b'image "' not in image_deleted and asset.read_bytes() == asset_original, image_deleted
    ax('元に戻す')
    assert gui.save() == image_added
    ax('やり直す')
    assert gui.save() == image_deleted
    ax('元に戻す')
    assert gui.save() == image_added
    checkpoint('all-content-kinds-image-delete-history')
    gui.press('＋ Slide')
    add('2列');gui.key('Escape')
    add('2列');gui.key('Escape')
    gui.save()
    ax('挿入')
    checkpoint('all-columns-visible')
    gui.key('Escape')
    for number in (1, 2):
        for side in ('左列', '右列'):
            parent = f'2列 {number} の{side}'
            add('本文', parent)
            gui.replace(f'{number}{side}')
            gui.key('Escape');gui.save()
    checkpoint('both-columns-edited')
    populated = gui.fixture.read_bytes()
    nested = re.findall(rb'columns\s*\{\s*column width=50\s*\{\s*text "([^"]*)"\s*\};\s*column width=50\s*\{\s*text "([^"]*)"\s*\}\s*\}', populated)
    assert nested == [(b'1' + '左列'.encode(), b'1' + '右列'.encode()), (b'2' + '左列'.encode(), b'2' + '右列'.encode())], populated
    ax('挿入');ax('2列 1 の左列')
    state = gui.act('get-app-state')
    assert re.search(r'button \(disabled\).*2列', state['snapshot']['treeText']), state['snapshot']['treeText']
    checkpoint('nested-columns-disabled')
    gui.key('Escape')
    ax('挿入')
    state = gui.act('get-app-state')
    assert re.search(r'radio button \([^)]*selected[^)]*\) スライド直下', state['snapshot']['treeText']), state['snapshot']['treeText']
    gui.key('Tab');gui.key('Return');gui.key('Escape')
    assert gui.save() == populated
    native = gui.out / 'native-menu-path'
    subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_menu_path.swift')), '-o', str(native)], check=True)
    gui.act('get-app-state', '--restore-window')
    subprocess.run([str(native), str(gui.app.pid), '挿入', 'スライド直下', '本文  説明を文章で伝える'], check=True)
    gui.act('get-app-state')
    gui.replace('native本文');gui.key('Escape')
    assert b'native' in gui.save()
    checkpoint('native-root-text')
    final = gui.fixture.read_bytes()
    gui.stop();gui.start()
    assert gui.fixture.read_bytes() == final
    checkpoint('saved-restart')
    gui.menu('プレゼンテーション', '最初からプレゼンテーション')
    checkpoint('presenting-content-kinds')
    gui.key('Right')
    checkpoint('presenting-columns')
    gui.key('Escape')
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n')
    gui.stop()
