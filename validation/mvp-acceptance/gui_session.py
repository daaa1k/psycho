"""Helpers for acceptance scripts operating only a fresh scratch PSYCHO process."""
import csv, json, os, shutil, subprocess, time, unicodedata
from pathlib import Path

root = Path(__file__).resolve().parents[2]

class Gui:
    def __init__(self, out, source, name='presentation'):
        self.out=Path(out).resolve();self.out.mkdir(parents=True,exist_ok=True)
        assert self.out.is_relative_to(root/'target'), 'Scratch files must be under target/'
        self.fixture=self.out/f'{name}.kdl'
        assert not self.fixture.exists(), 'Use a fresh directory; existing files are never replaced'
        self.fixture.write_text(source)
        self.original=self.fixture.read_bytes()
        self.input=self.out/f'{name}-input.csv';self.trace=self.out/f'{name}-glyphs.csv'
        self.ocr=self.out/'recognize-bounds'
        subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_bounds.swift')),'-o',str(self.ocr)],check=True)
        self.start()

    def start(self):
        self.app=subprocess.Popen([str(root/'target/debug/psycho'),str(self.fixture)],
            env=dict(os.environ,PSYCHO_INPUT_STATE=str(self.input),PSYCHO_GLYPH_TRACE=str(self.trace)),
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        time.sleep(3);self.act('get-app-state','--restore-window')

    def stop(self):
        self.app.terminate()
        try:self.app.wait(timeout=5)
        except subprocess.TimeoutExpired:self.app.kill();self.app.wait()

    def act(self,*args):
        result=subprocess.run(['orca','computer',*args,'--app',f'pid:{self.app.pid}','--json'],text=True,capture_output=True)
        value=json.loads(result.stdout)
        if not value['ok'] and args[0]=='click' and value.get('error',{}).get('code')=='window_not_focused':
            # A fullscreen/native-panel transition can deliver the press and
            # change the recipient before the provider verifies focus.
            # Observe once; the caller must assert the intended effect.
            return dict(unverified_click=value['error'],observation=self.act('get-app-state'))
        assert value['ok'],value
        time.sleep(.25)
        return value['result']

    def snapshot(self,name=None):
        time.sleep(.35);state=self.act('get-app-state')
        if name:shutil.copy(state['screenshot']['path'],self.out/f'{name}.png')
        rows=json.loads(subprocess.check_output([str(self.ocr),state['screenshot']['path']],text=True))
        return state,rows

    def press(self,label,scroll=False):
        for attempt in range(5 if scroll else 1):
            state,rows=self.snapshot()
            normalize=lambda text:unicodedata.normalize('NFKC',''.join(text.split())).replace('−','-').replace('–','-')
            matches=[r for r in rows if normalize(r['text'])==normalize(label)]
            if len(matches)==1:
                row=matches[0];return self.act('click','--x',str(row['x']),'--y',str(row['y']),'--no-screenshot')
            if not scroll:break
            self.act('scroll','--x',str(state['screenshot']['width']-130),'--y','710','--direction','down','--no-screenshot')
        raise AssertionError((label,rows))

    def key(self,key):return self.act('press-key','--key',key,'--no-screenshot')
    def hotkey(self,key):return self.act('hotkey','--key',key,'--no-screenshot')
    def click(self,x,y,count=1):return self.act('click','--x',str(x),'--y',str(y),'--click-count',str(count),'--no-screenshot')

    def state(self):
        time.sleep(.2);row=next(csv.reader(self.input.open()))
        return dict(text=bytes.fromhex(row[1]).decode(),selected=list(map(int,row[2:4])),marked=list(map(int,row[4:6])),
            cursor=list(map(float,row[6:9])),undo=int(row[9]),redo=int(row[10]),composing=row[11]=='true',focused=row[12]=='true')

    def replace(self,text):
        assert self.state()['focused'],'No focused scratch text input'
        self.hotkey('CmdOrCtrl+A')
        if text and ('\n' in text or '\t' in text):
            # Keep our known scratch payload on the native clipboard until the
            # asynchronous GPUI Cmd+V handler has consumed it.
            subprocess.run(['pbcopy'],input=text.encode(),check=True)
            self.hotkey('CmdOrCtrl+V')
        elif text:self.act('type-text','--text',text,'--no-screenshot')
        else:self.key('Backspace')
        assert self.state()['text']==text,self.state()

    def glyph_position(self,prefix,font=None):
        rows=list(csv.reader(self.trace.open()))
        row=next(r for r in reversed(rows) if len(r)==12 and r[4]=='0' and bytes.fromhex(r[3]).decode().startswith(prefix) and (font is None or float(r[2])==font))
        return float(row[6])+5,float(row[7])+32-5

    def edit(self,prefix,font=None):
        self.snapshot();x,y=self.glyph_position(prefix,font);self.click(x,y,2)

    def save(self):self.press('保存');return self.fixture.read_bytes()

    def text(self,name=None):
        _,rows=self.snapshot(name)
        return ''.join(''.join(r['text'].split()) for r in rows)
