import argparse
import json
import os
import re
import stat
import time
from pathlib import Path
from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Document workflow" }; slide { heading "Original"; }; }\n')
records = []
gui.stop()
source_dir = gui.out / "source"
source_dir.mkdir()
gui.fixture = source_dir / "presentation.kdl"
gui.fixture.write_bytes(gui.original)
gui.start()
mode = stat.S_IMODE(source_dir.stat().st_mode)

def tree():
    return gui.act('get-app-state')['snapshot']['treeText']

def click(name):
    state = tree()
    match = re.search(r'^\s*(\d+) button ' + re.escape(name) + r'$', state, re.M)
    assert match, (name, state)
    gui.act('click', '--element-index', match[1], '--no-screenshot')

def record(name):
    state, rows = gui.snapshot(name)
    records.append(dict(name=name, tree=state['snapshot']['treeText'],
                        text=''.join(row['text'] for row in rows), input=gui.state(),
                        source=gui.fixture.read_text() if gui.fixture.exists() else None))
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    return records[-1]

try:
    gui.edit('Original', 48)
    gui.replace('Draft preserved')
    assert '未保存' in record('dirty-draft')['text']
    gui.hotkey('CmdOrCtrl+W')
    modal = tree()
    assert all('button ' + label in modal for label in ('保存', '破棄', 'キャンセル')), modal
    record('native-close-confirmation')
    gui.key('Escape')
    assert gui.state()['text'] == 'Draft preserved'
    assert gui.state()['focused'], gui.state()
    assert gui.fixture.read_bytes() == gui.original
    record('close-cancel-focus')
    os.chmod(source_dir, 0o500)
    gui.press('保存')
    assert 'button 再試行' in tree(), tree()
    assert gui.fixture.read_bytes() == gui.original
    record('save-failure-modal')
    gui.key('Escape')
    assert '保存できませんでした' in record('persistent-save-failure')['text']
    assert gui.state()['focused'], gui.state()
    assert gui.state()['text'] == 'Draft preserved', gui.state()
    click('表示')
    gui.key('Escape')
    assert '保存できませんでした' in record('failure-after-view')['text']
    os.chmod(source_dir, mode)
    gui.press('保存')
    assert b'heading "Draft preserved"' in gui.fixture.read_bytes()
    assert '保存できませんでした' not in record('save-retry-success')['text']
    click('診断 0件')
    assert '診断' in tree()
    gui.key('Escape')
    gui.edit('Draft preserved', 48)
    gui.replace('Conflict retained')
    gui.fixture.write_text('presentation { metadata { title "External" }; slide { heading "External"; }; }\n')
    time.sleep(1.3)
    assert '競合' in record('external-conflict')['text']
    click('再読み込み')
    gui.key('Escape')
    assert gui.state()['text'] == 'Conflict retained'
    assert 'External' in gui.fixture.read_text()
    record('reload-cancel')
    click('再読み込み')
    click('破棄して再読み込み')
    time.sleep(.8)
    assert gui.state()['text'] == 'External'
    assert '外部ファイルに問題があります' not in record('reload-success')['text']
    gui.edit('External', 48)
    gui.replace('Retained valid canvas')
    gui.fixture.write_text('presentation { slide { heading } }\n')
    time.sleep(1.3)
    invalid = record('external-invalid')
    assert '外部ファイルが無効' in invalid['text'], invalid
    assert invalid['input']['text'] == 'Retained valid canvas', invalid
    assert 'Retained valid canvas' in invalid['text'], invalid
    assert re.search(r'button 診断 [1-9]\d*件', tree()), tree()
    label = re.search(r'button (診断 \d+件)', tree())[1]
    click(label)
    record('diagnostics-list')
    gui.key('Escape')
    gui.fixture.unlink()
    click('再読み込み')
    click('破棄して再読み込み')
    time.sleep(.8)
    assert '削除されています' in record('external-deleted')['text']
    archive = gui.out / 'archive'
    archive.mkdir()
    gui.button('別名保存')
    click('退避先を選ぶ')
    gui.hotkey('CmdOrCtrl+Shift+G')
    gui.hotkey('CmdOrCtrl+A')
    gui.act('type-text', '--text', str(archive), '--no-screenshot')
    gui.key('Return')
    click('Save')
    saved = archive / 'presentation.kdl'
    assert b'heading "Retained valid canvas"' in saved.read_bytes()
    assert not gui.fixture.exists()
    gui.hotkey('CmdOrCtrl+Z')
    gui.press('保存')
    assert b'heading "Retained valid canvas"' in saved.read_bytes()
    record('retreat-preserves-draft-clears-history')
    print('PASS document status, native cancel/focus, persistent failure, retry, conflict, reload, invalid and deleted source')
finally:
    os.chmod(source_dir, mode)
    gui.stop()
