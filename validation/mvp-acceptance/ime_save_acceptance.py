import argparse
import json
from pathlib import Path

from gui_session import Gui

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--entry', choices=['menu', 'shortcut', 'confirmed-shortcut'], default='menu')
args = parser.parse_args()
gui = Gui(args.output, 'presentation { metadata { title "IME save"; }; slide { heading "Original"; }; }\n')
records = []
try:
    main_window_id = gui.act('get-app-state')['snapshot']['window']['id']
    gui.edit('Original', 48)
    gui.hotkey('CmdOrCtrl+A')
    for key in 'nihongonohenkann':
        gui.key(key)
    gui.key('Space')
    marked = gui.state()
    records.append(dict(name='marked', input=marked, disk_hex=gui.fixture.read_bytes().hex()))
    gui.snapshot('marked-before-save')
    assert marked['composing'] and marked['marked'][0] >= 0, marked
    assert marked['text'].startswith('日本語の'), marked
    expected = gui.original.replace(b'Original', marked['text'].encode())
    if args.entry == 'menu':
        gui.menu('ファイル', '閉じる')
        gui.snapshot('close-confirmation')
        gui.key('Escape')
        cancelled = gui.state()
        records.append(dict(name='close-cancel', input=cancelled,
                            disk_hex=gui.fixture.read_bytes().hex()))
        assert cancelled['text'] == marked['text'] and cancelled['focused'], cancelled
        assert gui.fixture.read_bytes() == gui.original
        gui.snapshot('close-cancelled')
        gui.menu('ファイル', '保存')
    else:
        if args.entry == 'confirmed-shortcut':
            gui.key('Enter')
            gui.key('Enter')
            assert not gui.state()['composing'], gui.state()
            assert gui.fixture.read_bytes() == gui.original
        gui.act('hotkey', '--key', 'CmdOrCtrl+S', '--window-id', str(main_window_id), '--no-screenshot')
    gui.snapshot('after-save')
    saved = gui.state()
    records.append(dict(name='save', entry=args.entry, input=saved,
                        disk_hex=gui.fixture.read_bytes().hex(), expected_hex=expected.hex()))
    assert gui.fixture.read_bytes() == expected, records[-1]
    assert not saved['composing'] and saved['marked'][0] < 0, saved
    assert not saved['focused'], saved
    print('PASS native Japanese IME save via ' + args.entry, flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
    gui.stop()
