"""Exercise Apple's Japanese IME in a fresh, owned PSYCHO window.

Select Apple's Japanese Romaji input source before running.
All file edits are confined to a newly generated target directory.
"""
import argparse, csv, json, os, shutil, subprocess, time
from pathlib import Path

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root/'target/mvp-ime-evidence')
out = parser.parse_args().output.resolve()
out.mkdir(parents=True, exist_ok=True)
fixture = out/'presentation.kdl'
original = b'presentation { metadata { title "IME position" }; slide { heading "IME acceptance"; text "original body"; }; }\n'
assert not fixture.exists(), 'Use a fresh output directory; edited fixtures are never overwritten'
fixture.write_bytes(original)
state_file, rect_file = out/'input-state.csv', out/'character-rects.csv'
app = subprocess.Popen([str(root/'target/debug/psycho'), str(fixture)],
    env=dict(os.environ, PSYCHO_INPUT_STATE=str(state_file), PSYCHO_IME_RECTS=str(rect_file)),
    stdout=subprocess.DEVNULL, stderr=(out/'app-stderr.txt').open('w'))
observations = []
ocr = out/'recognize-bounds'
subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_bounds.swift')),'-o',str(ocr)],check=True)

def act(*args):
    result = json.loads(subprocess.check_output(['orca', 'computer', *args, '--app', f'pid:{app.pid}', '--json'], text=True))
    assert result['ok'], result
    time.sleep(.25)  # Native IME callbacks and GPUI paint are asynchronous.
    return result['result']

def input_state():
    row = next(csv.reader(state_file.open()))
    return dict(text=bytes.fromhex(row[1]).decode(), selected=list(map(int,row[2:4])),
        marked=list(map(int,row[4:6])), cursor=list(map(float,row[6:9])),
        undo=int(row[9]), redo=int(row[10]), composing=row[11]=='true', focused=row[12]=='true')

def windows():
    return json.loads(subprocess.check_output(['xcrun','swift',str(Path(__file__).with_name('ime_windows.swift')),str(app.pid)],text=True))

def record(name, screenshot=False):
    time.sleep(.6)
    value = dict(name=name, input=input_state(), windows=windows())
    observations.append(value)
    if screenshot:
        main=next(w for w in value['windows'] if w['layer']==0 and w['bounds']['Width']>=1000)
        captured=act('get-app-state','--window-id',str(main['id']))
        shutil.copy(captured['screenshot']['path'],out/f'{name}-editor.png')
        panels=[w for w in value['windows'] if w['layer']==20]
        if panels:
            panel=min(panels,key=lambda w:w['bounds']['X'])
            # screencapture preserves the native ViewBridge candidate group.
            # Orca's per-window ScreenCaptureKit capture scales it incorrectly.
            subprocess.run(['screencapture','-l',str(panel['id']),'-x',str(out/f'{name}-candidate-group.png')],check=True,capture_output=True)
        # Capture only the foreground scratch editor's screen rectangle so its
        # native candidate panel is visible at its actual screen position.
        subprocess.run(['xcrun','swift','-e','import Cocoa; precondition(NSRunningApplication(processIdentifier:pid_t(CommandLine.arguments[1])!)?.isActive == true)',str(app.pid)],check=True,capture_output=True)
        b=main['bounds'];region=','.join(str(round(b[k])) for k in ['X','Y','Width','Height'])
        subprocess.run(['screencapture','-R',region,'-x',str(out/f'{name}-visible.png')],check=True,capture_output=True)
    print(name, json.dumps(value['input'],ensure_ascii=False),flush=True)
    return value

def press_label(label):
    main=next(w for w in windows() if w['layer']==0 and w['bounds']['Width']>=1000)
    state=act('get-app-state','--window-id',str(main['id']))
    rows=json.loads(subprocess.check_output([str(ocr),state['screenshot']['path']],text=True))
    matches=[row for row in rows if ''.join(row['text'].split())==''.join(label.split())]
    assert len(matches)==1,(label,rows)
    row=matches[0];act('click','--x',str(row['x']),'--y',str(row['y']),'--no-screenshot')

def composition():
    act('hotkey','--key','CmdOrCtrl+A','--no-screenshot')
    for key in 'nihongonohenkann': act('press-key','--key',key,'--no-screenshot')
    act('press-key','--key','Space','--no-screenshot')
    act('press-key','--key','Down','--no-screenshot')
    # IME learning can change the next candidate. Select the intended native
    # candidate using navigation, without resetting the user's IME dictionary.
    for attempt in range(10):
        if input_state()['text']=='日本語の変換': break
        act('press-key','--key','Up','--no-screenshot')
    else: raise AssertionError('Intended native candidate was not available')

def same_conversion(before,after):
    for key in ['text','selected','marked','undo','redo','composing','focused']:
        assert before['input'][key]==after['input'][key],(key,before,after)

def follows(before,after,dx,dy):
    panels=[w for w in before['windows'] if w['layer']==20]
    assert panels,'Native candidate panel not shown'
    for old in panels:
        new=next(w for w in after['windows'] if w['id']==old['id'])
        assert abs(new['bounds']['X']-old['bounds']['X']-dx)<=1,(old,new,dx,dy)
        assert abs(new['bounds']['Y']-old['bounds']['Y']-dy)<=1,(old,new,dx,dy)
    same_conversion(before,after)

def selected_rect():
    row=list(csv.reader(rect_file.open()))[-1]
    return tuple(map(float,row[3:7]))

try:
    time.sleep(3)
    act('get-app-state','--restore-window')
    act('click','--x','300','--y','316','--click-count','2','--no-screenshot')
    composition()
    initial=record('ime-candidates',True)
    assert initial['input']['text']=='日本語の変換',initial
    assert initial['input']['marked']==[0,6] and initial['input']['undo']==0
    assert initial['input']['focused'] and initial['input']['composing']
    subprocess.run(['xcrun','swift',str(Path(__file__).with_name('move_window.swift')),str(app.pid),'500','255'],check=True,capture_output=True)
    moved=record('ime-window-move',True); follows(initial,moved,180,70)
    subprocess.run(['xcrun','swift',str(Path(__file__).with_name('slow_drag.swift')),str(app.pid),'200','16','300','46','0'],check=True,capture_output=True)
    dragged=record('ime-titlebar-drag',True); follows(moved,dragged,100,30)
    act('press-key','--key','Right','--no-screenshot')
    right=record('ime-right',True)
    assert right['input']['text']==initial['input']['text'] and right['input']['marked']==[0,6]
    assert right['input']['selected']!=initial['input']['selected'] and right['input']['undo']==0
    act('press-key','--key','Left','--no-screenshot')
    left=record('ime-left'); assert left['input']['text']==initial['input']['text'] and left['input']['undo']==0
    subprocess.run(['xcrun','swift',str(Path(__file__).with_name('move_window.swift')),str(app.pid),'320','185'],check=True,capture_output=True)
    before=record('ime-before-resize'); old_rect=selected_rect()
    subprocess.run(['xcrun','swift',str(Path(__file__).with_name('resize_window.swift')),str(app.pid),'1440','992'],check=True,capture_output=True)
    resized=record('ime-resize',True); new_rect=selected_rect()
    follows(before,resized,new_rect[0]-old_rect[0],new_rect[1]+new_rect[3]-old_rect[1]-old_rect[3])
    # With the candidate list open, Return can first choose the candidate and
    # leave the conversion session active. Only IME decides when it is final.
    for attempt in range(1,4):
        act('press-key','--key','Return','--no-screenshot')
        confirmed=record(f'ime-return-{attempt}',True)
        assert confirmed['input']['text']=='日本語の変換'
        if confirmed['input']['marked']==[-1,-1]:
            break
        assert confirmed['input']['undo']==0 and confirmed['input']['focused']
    else:
        raise AssertionError('Native IME did not finalize after selecting the candidate')
    assert confirmed['input']['undo']==1 and not confirmed['input']['composing']
    act('press-key','--key','Return','--no-screenshot')
    newline=record('ime-newline'); assert newline['input']['text']=='日本語の変換\n' and newline['input']['undo']==2
    act('hotkey','--key','CmdOrCtrl+Z','--no-screenshot'); undone=record('ime-undo-newline')
    assert undone['input']['text']=='日本語の変換' and undone['input']['undo']==1
    act('hotkey','--key','CmdOrCtrl+Z','--no-screenshot'); undone=record('ime-undo-conversion')
    assert undone['input']['text']=='original body' and undone['input']['undo']==0
    act('hotkey','--key','CmdOrCtrl+Shift+Z','--no-screenshot'); redone=record('ime-redo-conversion')
    assert redone['input']['text']=='日本語の変換' and redone['input']['undo']==1
    assert fixture.read_bytes()==original, 'IME and window operations must not save automatically'
    act('press-key','--key','Escape','--no-screenshot')
    act('click','--x','85','--y','55','--no-screenshot')
    assert fixture.read_bytes()==original.replace(b'original body','日本語の変換'.encode())
    record('ime-saved',True)
    # Start another edit and keep its native composition while moving the same
    # input entity from Canvas to Inspector. Escape must first reach the IME.
    act('click','--x','260','--y','346','--click-count','2','--no-screenshot')
    composition()
    before_switch=record('ime-before-inspector',True)
    press_label('全文をInspectorで編集')
    switched=record('ime-inspector',True)
    same_conversion(before_switch,switched)
    assert switched['input']['cursor'][0]>1170, 'Input was not mounted in Inspector'
    for panel in switched['windows']:
        if panel['layer']==20:
            b=panel['bounds'];assert b['X']>=0 and b['X']+b['Width']<=1920 and b['Y']>=0 and b['Y']+b['Height']<=1200,b
    act('click','--x','260','--y','346','--click-count','2','--no-screenshot')
    returned=record('ime-return-to-canvas',True)
    same_conversion(before_switch,returned)
    assert returned['input']['cursor'][0]<400, 'Input was not mounted back in Canvas'
    follows(before_switch,returned,0,0)
    press_label('全文をInspectorで編集')
    record('ime-inspector-again',True)
    act('press-key','--key','Escape','--no-screenshot')
    escaped=record('ime-escape-priority',True)
    assert escaped['input']['text']=='にほんごのへんかん'
    assert escaped['input']['marked'][0]>=0 and escaped['input']['focused'] and escaped['input']['composing']
    assert escaped['input']['undo']==before_switch['input']['undo']
    press_label('入力を取り消す')
    cancelled=record('ime-cancelled',True)
    assert cancelled['input']['text']=='日本語の変換' and cancelled['input']['marked']==[-1,-1]
    assert cancelled['input']['undo']==0 and not cancelled['input']['composing']
    assert fixture.read_bytes()==original.replace(b'original body','日本語の変換'.encode())
    print('PASS native geometry, IME key priority, Canvas/Inspector switch, Cancel, Undo/Redo and Save',flush=True)
finally:
    (out/'ime-observations.json').write_text(json.dumps(observations,ensure_ascii=False,indent=2)+'\n')
    app.terminate()
    try: app.wait(timeout=5)
    except subprocess.TimeoutExpired: app.kill(); app.wait()
