import argparse,json,subprocess,time
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-layout-repair')
large='overflow();\n'*30
short='repaired();'
source='presentation { metadata { title "Layout repair" }; slide id="good" { heading "Good slide"; }; slide id="bad" { heading "Offscreen code"; code '+json.dumps(large)+'; }; }\n'
output=parser.parse_args().output
gui=Gui(output,source)
try:
    assert gui.save()==gui.original,'Overflowing content remains saveable'
    for label in ['最初から発表','現在から発表']:
        gui.press(label)
        state,rows=gui.snapshot('noncurrent-blocked-'+str(len(label)))
        assert state['screenshot']['width']==1280
        assert any(166<row['x']<995 and row['text']=='Good slide' for row in rows)
    _,rows=gui.snapshot()
    row=next(row for row in rows if 'Layout' in row['text'] and 'Element' in row['text'])
    gui.click(row['x'],row['y'])
    assert gui.state()['text']==large,'Diagnostic must select the complete overflowing Element'
    _,rows=gui.snapshot('offscreen-full-inspector')
    row=next(row for row in rows if row['x']>1000 and 'overflow' in row['text'])
    gui.click(row['x'],row['y']);gui.hotkey('CmdOrCtrl+A');gui.hotkey('CmdOrCtrl+C')
    assert subprocess.check_output(['pbpaste']).decode()==large,'Inspector must retain the whole offscreen value'
    gui.replace(short)
    _,rows=gui.snapshot('overflow-repaired-preview')
    assert not any('Layout' in row['text'] and 'Element' in row['text'] for row in rows)
    assert gui.fixture.read_bytes()==gui.original,'Preview repair must not save automatically'
    saved=gui.save();assert saved.decode()==source.replace(json.dumps(large),json.dumps(short))
    gui.press('Undo');assert gui.save()==gui.original
    gui.press('Redo');assert gui.save()==saved
    gui.press('最初から発表');gui.key('Right')
    state,rows=gui.snapshot('repaired-presentation')
    assert state['screenshot']['width']==1920 and any('repaired' in row['text'] for row in rows)
    assert gui.fixture.read_bytes()==saved
    gui.key('Escape');time.sleep(1)
    print('PASS noncurrent overflow blocks both starts, full Inspector repair, immediate diagnosis clearing, Save, exact Undo/Redo and presentation',flush=True)
except Exception:
    gui.snapshot('layout-repair-failure');raise
finally:gui.stop()

nested_source='presentation { metadata { title "Nested outline" }; slide { code '+json.dumps(large)+'; columns { column width=50 { text "Left hidden"; }; column width=50 { text "Right hidden"; }; }; }; }\n'
gui=Gui(output/'nested',nested_source)
try:
    _,rows=gui.snapshot('offscreen-columns')
    assert not any(166<row['x']<995 and row['text'] in ('Left hidden','Right hidden') for row in rows)
    gui.press('左列 · 本文 1')
    assert gui.state()['text']=='Left hidden'
    gui.press('右列 · 本文 1')
    assert gui.state()['text']=='Right hidden'
    _,rows=gui.snapshot()
    row=next(row for row in rows if row['x']>1000 and 'Right hidden' in row['text'])
    gui.click(row['x'],row['y'])
    gui.replace('Right repaired')
    saved=gui.save()
    assert saved.decode()==nested_source.replace('Right hidden','Right repaired')
    gui.stop();gui.start()
    gui.press('右列 · 本文 1')
    assert gui.state()['text']=='Right repaired'
    print('PASS offscreen nested Elements on both sides, repair, Save and reload',flush=True)
except Exception:
    gui.snapshot('nested-outline-failure');raise
finally:gui.stop()

outline_source='presentation { metadata { title "Outline repair" }; slide { code '+json.dumps(large)+'; text "Hidden text"; }; }\n'
gui=Gui(output/'outline',outline_source)
try:
    _,rows=gui.snapshot('offscreen-element')
    assert not any(166<row['x']<995 and row['text']=='Hidden text' for row in rows),'The text Element must be offscreen'
    gui.press('Element 1 · コード')
    _,rows=gui.snapshot()
    row=next(row for row in rows if row['x']>1000 and 'overflow' in row['text'])
    gui.click(row['x'],row['y'])
    gui.replace('pending();')
    gui.press('Element 2 · 本文')
    assert gui.state()['text']=='Hidden text','Outline selection must load the complete offscreen Element'
    _,rows=gui.snapshot()
    row=next(row for row in rows if row['x']>1000 and 'Hidden text' in row['text'])
    gui.click(row['x'],row['y'])
    gui.replace('Recovered text')
    saved=gui.save()
    expected=outline_source.replace(json.dumps(large),json.dumps('pending();')).replace('Hidden text','Recovered text')
    assert saved.decode()==expected,'Changing outline selection must commit the previous draft'
    gui.stop();gui.start()
    gui.press('Element 2 · 本文')
    assert gui.state()['text']=='Recovered text','The offscreen edit must survive reload'
    assert 'Element2' in gui.text('outline-reloaded'),'Outline must remain available after selection'
    print('PASS offscreen outline selection, draft commit on selection, editing, Save and reload',flush=True)
except Exception:
    gui.snapshot('outline-failure');raise
finally:gui.stop()
