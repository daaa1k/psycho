import argparse,ctypes,json,shutil,time,subprocess
from pathlib import Path
from gui_session import Gui,root

parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=root/'target/mvp-close-cursor')
parser.add_argument('--cursor-only',action='store_true');args=parser.parse_args()
out=args.output;out.mkdir(parents=True,exist_ok=True)
native=out/'native-windows'
subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('native_windows.swift')),'-o',str(native)],check=True)
source=(root/'examples/build-time.kdl').read_text().replace('../assets/build-time.png','assets/build-time.png')
records=[]
api=ctypes.CDLL('/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices')
api.CGCursorIsVisible.restype=ctypes.c_bool
api.CGEventCreate.argtypes=[ctypes.c_void_p];api.CGEventCreate.restype=ctypes.c_void_p
class Point(ctypes.Structure):_fields_=[('x',ctypes.c_double),('y',ctypes.c_double)]
api.CGEventGetLocation.argtypes=[ctypes.c_void_p];api.CGEventGetLocation.restype=Point
api.CGEventCreateMouseEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint32,Point,ctypes.c_uint32];api.CGEventCreateMouseEvent.restype=ctypes.c_void_p
api.CGEventPost.argtypes=[ctypes.c_uint32,ctypes.c_void_p]
api.CGEventSetIntegerValueField.argtypes=[ctypes.c_void_p,ctypes.c_uint32,ctypes.c_int64]
api.CGEventSourceCreate.argtypes=[ctypes.c_int32];api.CGEventSourceCreate.restype=ctypes.c_void_p
api.CFRelease.argtypes=[ctypes.c_void_p]
def visible():return bool(api.CGCursorIsVisible())
def move():
    event=api.CGEventCreate(None);p=api.CGEventGetLocation(event);api.CFRelease(event)
    source=api.CGEventSourceCreate(1)
    event=api.CGEventCreateMouseEvent(source,5,Point(800 if p.x<800 else 700,400),0);api.CGEventSetIntegerValueField(event,4,100 if p.x<800 else -100);api.CGEventSetIntegerValueField(event,5,20);api.CGEventPost(0,event);api.CFRelease(event);api.CFRelease(source)
    time.sleep(.03);event=api.CGEventCreate(None);after=api.CGEventGetLocation(event);api.CFRelease(event)
    print('MOUSE',p.x,p.y,'to',after.x,after.y,'visible',visible(),flush=True)
def close_request(gui):
    state=gui.act('get-app-state');(gui.out/'window-before-close.json').write_text(json.dumps(state,ensure_ascii=False,indent=2))
    native_before=json.loads(subprocess.check_output([str(native),str(gui.app.pid)],text=True))
    gui.document_window_ids={w['kCGWindowNumber'] for w in native_before if w['kCGWindowBounds']['Width']==state['snapshot']['window']['width'] and w['kCGWindowBounds']['Height']>700}
    assert gui.document_window_ids,native_before
    (gui.out/'native-windows-before.json').write_text(json.dumps(native_before,ensure_ascii=False,indent=2))
    gui.click(20,13)
    assert '保存して閉じる' in gui.text('unsaved-close-dialog')
for branch in ([] if args.cursor_only else ['cancel','save','discard']):
    gui=Gui(out/branch,source)
    assets=gui.out/'assets';assets.mkdir();shutil.copy(root/'assets/build-time.png',assets/'build-time.png')
    try:
        gui.stop();gui.start();gui.edit('CI のビルドが遅い',48)
        new='CLOSE ACCEPTANCE '+branch;gui.replace(new)
        close_request(gui)
        if branch=='cancel':
            gui.press('閉じるのをやめる');assert gui.app.poll() is None
            assert gui.state()['text']==new,gui.state()
            assert new.replace(' ','') in gui.text('cancel-retains-input')
            assert gui.fixture.read_bytes()==gui.original
            records.append(dict(branch=branch,input=gui.state(),alive=True,original_bytes=True))
        else:
            try:gui.press('保存して閉じる' if branch=='save' else '破棄して閉じる',aliases=('破楽して閉じる',) if branch=='discard' else ())
            except AssertionError as error:
                (gui.out/'close-action-observation.txt').write_text(str(error))
            time.sleep(2);result=gui.app.poll();actual=gui.fixture.read_bytes()
            windows=json.loads(subprocess.check_output([str(native),str(gui.app.pid)],text=True));assert gui.document_window_ids.isdisjoint({w['kCGWindowNumber'] for w in windows}),windows
            (gui.out/'windows-after-close.json').write_text(json.dumps(windows,ensure_ascii=False,indent=2))
            expected=gui.original.replace('CI のビルドが遅い'.encode(),new.encode()) if branch=='save' else gui.original
            assert actual==expected,(branch,actual.decode())
            (gui.out/'expected.kdl').write_bytes(expected)
            records.append(dict(branch=branch,exit_code=result,window_closed=True,exact_expected_bytes=True,actual_hex=actual.hex()))
        print('PASS close',branch,flush=True)
    finally:gui.stop()
gui=Gui(out/'cursor',source)
assets=gui.out/'assets';assets.mkdir();shutil.copy(root/'assets/build-time.png',assets/'build-time.png')
try:
    gui.stop();gui.start();gui.press('最初から発表');time.sleep(1)
    move();time.sleep(.15);assert visible(),'Cursor must initially be visible'
    initial=visible();start=time.monotonic();time.sleep(2.15);hidden=visible();idle_elapsed=time.monotonic()-start
    assert not hidden,'Cursor remains visible after presentation idle'
    move();time.sleep(.35);redisplayed=visible();assert redisplayed
    before=gui.text('mousemove-visible');assert 'CIのビルドが遅い' in before
    gui.click(600,350);assert 'CIのビルドが遅い' in gui.text('click-keeps-slide')
    gui.act('scroll','--x','600','--y','350','--direction','down','--no-screenshot')
    assert 'CIのビルドが遅い' in gui.text('scroll-keeps-slide')
    time.sleep(2.15);assert not visible();gui.key('Escape');time.sleep(.6);restored=visible();assert restored
    gui.snapshot('exit-restores-cursor')
    records.append(dict(branch='cursor',native_api='CGCursorIsVisible',initial_visible=initial,idle_elapsed_seconds=idle_elapsed,idle_visible=hidden,mousemove_visible=redisplayed,click_keeps_slide=True,scroll_keeps_slide=True,exit_visible=restored))
    print('PASS presentation native cursor idle/move/exit and click/scroll',flush=True)
finally:gui.stop()
(out/'observations.json').write_text(json.dumps(records,ensure_ascii=False,indent=2))
print('RESULT',json.dumps(records,ensure_ascii=False),flush=True)
print('All scratch processes stopped; last-window close permits macOS process retention',flush=True)
