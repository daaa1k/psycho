"""Helpers for acceptance scripts operating only a fresh scratch PSYCHO process."""
import csv, json, os, re, shutil, subprocess, time, unicodedata
from pathlib import Path

root = Path(__file__).resolve().parents[2]

class Gui:
    def __init__(self, out, source, name='presentation'):
        self.out=Path(out).resolve();self.out.mkdir(parents=True,exist_ok=True)
        assert self.out.is_relative_to((root/'target').resolve()), 'Scratch files must be under target/'
        self.fixture=self.out/f'{name}.kdl'
        assert not self.fixture.exists(), 'Use a fresh directory; existing files are never replaced'
        self.fixture.write_text(source)
        self.original=self.fixture.read_bytes()
        self.input=self.out/f'{name}-input.csv';self.trace=self.out/f'{name}-glyphs.csv'
        self.ocr=self.out/'recognize-bounds'
        subprocess.run(['xcrun','swiftc',str(Path(__file__).with_name('recognize_bounds.swift')),'-o',str(self.ocr)],check=True)
        try:self.start()
        except Exception:self.stop();raise

    def start(self):
        self.launch_number=getattr(self,'launch_number',0)+1
        self.product_log=self.out/f'product-{self.launch_number}.log'
        with self.product_log.open('w') as log:
            self.app=subprocess.Popen([str(root/'target/debug/psycho'),str(self.fixture)],
                env=dict(os.environ,PSYCHO_INPUT_STATE=str(self.input),PSYCHO_GLYPH_TRACE=str(self.trace),RUST_BACKTRACE='1'),
                stdout=subprocess.DEVNULL,stderr=log)
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            assert self.app.poll() is None,'Scratch app exited during startup'
            available=json.loads(subprocess.check_output(['orca','computer','list-apps','--json'],text=True))
            assert available['ok'],available
            if any(app['pid']==self.app.pid for app in available['result']['apps']):break
            time.sleep(.5)
        else:raise AssertionError('Scratch app did not register a GUI within 15 seconds')
        time.sleep(1);self.act('get-app-state','--restore-window')
        self.act('click','--x','900','--y','750','--no-screenshot')
        self.recorder=None
        executable=os.environ.get('PSYCHO_ACCEPTANCE_RECORDER')
        if executable:
            log=self.out/f'window-{self.app.pid}.log'
            with log.open('w') as output:
                self.recorder=subprocess.Popen([executable,str(self.app.pid),
                    str(self.out/f'window-{self.app.pid}.mov'),'3600'],stdout=output,stderr=subprocess.STDOUT)
            deadline=time.monotonic()+15
            while 'RECORDING' not in log.read_text():
                assert self.recorder.poll() is None,log.read_text()
                assert time.monotonic()<deadline,'Window recording did not start'
                time.sleep(.1)

    def stop(self):
        recorder=getattr(self,'recorder',None)
        try:
            if recorder:
                recorder.terminate()
                recorder.wait(timeout=20)
                self.recorder=None
                assert recorder.returncode==0,'Window recording did not finish successfully'
        finally:
            self.app.terminate()
            try:self.app.wait(timeout=5)
            except subprocess.TimeoutExpired:self.app.kill();self.app.wait()

    def act(self,*args):
        result=subprocess.run(['orca','computer',*args,'--app',f'pid:{self.app.pid}','--json'],text=True,capture_output=True)
        value=json.loads(result.stdout)
        if not value['ok'] and value.get('error',{}).get('code')=='app_not_found' and args==('get-app-state',):
            evidence=dict(pid=self.app.pid,original=value,returncode=self.app.poll(),retried=False)
            try:
                if evidence['returncode'] is None:
                    available=subprocess.run(['orca','computer','list-apps','--json'],text=True,capture_output=True,timeout=15)
                    evidence['list_apps']=dict(returncode=available.returncode)
                    listed=json.loads(available.stdout)
                    evidence['list_apps'].update(ok=listed.get('ok'),owned_pid_present=any(
                        app.get('pid')==self.app.pid for app in listed.get('result',{}).get('apps',[])))
                    if available.returncode==0 and listed.get('ok') and any(
                        app.get('pid')==self.app.pid for app in listed.get('result',{}).get('apps',[])
                    ) and self.app.poll() is None:
                        evidence['retried']=True
                        result=subprocess.run(['orca','computer',*args,'--app',f'pid:{self.app.pid}','--json'],text=True,capture_output=True,timeout=15)
                        evidence['retry']=dict(returncode=result.returncode)
                        value=json.loads(result.stdout)
                        evidence['retry'].update(ok=value.get('ok'),error=value.get('error'))
            except (subprocess.TimeoutExpired,ValueError,AttributeError,TypeError) as error:
                evidence['recovery_error']=str(error)
            finally:
                evidence['returncode']=self.app.poll()
                with (self.out/'observation-recovery.jsonl').open('a') as log:
                    log.write(json.dumps(evidence)+'\n')
            assert evidence['returncode'] is None,dict(observation=evidence,product_log=str(self.product_log))
        if not value['ok'] and value.get('error',{}).get('code')=='window_not_focused' and args[0] in ('press-key','hotkey'):
            self.act('get-app-state','--restore-window')
            result=subprocess.run(['orca','computer',*args,'--app',f'pid:{self.app.pid}','--json'],text=True,capture_output=True)
            value=json.loads(result.stdout)
        if not value['ok'] and args[0]=='click' and value.get('error',{}).get('code')=='window_not_focused':
            # A fullscreen/native-panel transition can deliver the press and
            # change the recipient before the provider verifies focus.
            # Observe once; the caller must assert the intended effect.
            return dict(unverified_click=value['error'],observation=self.act('get-app-state'))
        assert value['ok'],dict(error=value,returncode=self.app.poll(),product_log=str(self.product_log))
        time.sleep(.25)
        return value['result']

    def snapshot(self,name=None):
        time.sleep(.35);state=self.act('get-app-state')
        if name:shutil.copy(state['screenshot']['path'],self.out/f'{name}.png')
        rows=json.loads(subprocess.check_output([str(self.ocr),state['screenshot']['path']],text=True))
        scale=state['screenshot'].get('scale',1)
        for row in rows:
            for key in ('x','y','width','height'):
                row[key]/=scale
        return state,rows

    def menu(self, category, title):
        helper = self.out / 'native-menu'
        if not helper.exists():
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('edit_menu_state.swift')),
                            '-o', str(helper)], check=True)
        self.act('get-app-state', '--restore-window')
        subprocess.run([str(helper), str(self.app.pid), category], check=True, stdout=subprocess.DEVNULL)
        if title in ('元に戻す', 'やり直す'):
            metadata = json.loads(subprocess.check_output([str(helper), str(self.app.pid)], text=True))
            def nodes(node):
                yield node
                for child in node.get('children', []):
                    yield from nodes(child)
            title = next(node['title'] for node in nodes(metadata['menuBar'])
                         if node.get('title', '').startswith(title))
        subprocess.run([str(helper), str(self.app.pid), title], check=True, stdout=subprocess.DEVNULL)
        self.act('get-app-state')

    def press(self,label,scroll=False,aliases=()):
        if label in ('Undo', 'Redo', '挿入', '見出し', '本文', '箇条書き', 'コード', '画像', '2列'):
            label = {'Undo': '元に戻す', 'Redo': 'やり直す'}.get(label, label)
            state = self.act('get-app-state')
            match = next(re.finditer(r'^\s*(\d+) button (\(disabled\) )?' + re.escape(label) + r'$',
                                     state['snapshot']['treeText'], re.MULTILINE), None)
            assert match, state['snapshot']['treeText']
            if match.group(2):
                return state
            return self.act('click', '--element-index', match.group(1), '--no-screenshot')
        actions = {
            '開く': ('ファイル', '開く'), '保存': ('ファイル', '保存'), '別名保存': ('ファイル', '別名保存'),
                        '＋ Slide': ('スライド', 'スライド追加'), '− Slide': ('スライド', 'スライドを削除'),
            'Slide ↑': ('スライド', '前へ移動'), 'Slide ↓': ('スライド', '後ろへ移動'),
            '現在から発表': ('プレゼンテーション', '現在からプレゼンテーション'), '最初から発表': ('プレゼンテーション', '最初からプレゼンテーション'),
        }
        if label in actions:
            return self.menu(*actions[label])
        for attempt in range(5 if scroll else 1):
            state,rows=self.snapshot()
            normalize=lambda text:unicodedata.normalize('NFKC',''.join(text.split())).replace('−','-').replace('–','-').replace('・','').replace('·','')
            accepted={normalize(text) for text in (label,*aliases)}
            matches=[r for r in rows if normalize(r['text']) in accepted]
            if len(matches)==1:
                row=matches[0];return self.act('click','--x',str(row['x']),'--y',str(row['y']),'--no-screenshot')
            if not scroll:break
            self.act('scroll','--x',str(state['snapshot']['window']['width']-130),'--y','710','--direction','down','--no-screenshot')
        raise AssertionError((label,rows))

    def scroll_at(self, x, y, direction):
        helper = self.out / 'scroll-at'
        if not helper.exists():
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('scroll_at.swift')),
                            '-o', str(helper)], check=True)
        subprocess.run([str(helper), str(self.app.pid), str(x), str(y),
                        '-160' if direction == 'up' else '160'], check=True)

    def inspector_scroll(self, y, direction):
        window = self.act('get-app-state')['snapshot']['window']
        self.scroll_at(window['width'] - 150, y, direction)

    def outline_select(self, label):
        for direction in ('down', 'up'):
            for _ in range(24):
                tree = self.act('get-app-state')['snapshot']['treeText']
                match = re.search(r'^\s*(\d+) text (?:\([^)]*\) )?' + re.escape(label) +
                                  r'(?:, .*)?$', tree, re.M)
                if match:
                    self.act('click', '--element-index', match[1], '--no-screenshot')
                    return
                self.inspector_scroll(265, direction)
        raise AssertionError((label, tree))

    def key(self,key):return self.act('press-key','--key',key,'--no-screenshot')
    def select_slide(self,number):
        for _ in range(20):
            tree = self.act('get-app-state')['snapshot']['treeText']
            match = re.search(r'^\s*(\d+) text (?:\([^)]*\) )?スライド ' + str(number) +
                              r'(?!\d)(?: .*|, .*)?$', tree, re.M)
            if match:
                return self.act('click', '--element-index', match[1], '--no-screenshot')
            visible = [int(n) for n in re.findall(r'text (?:\([^)]*\) )?スライド (\d+)', tree)]
            direction = 'up' if visible and number < min(visible) else 'down'
            window = self.act('get-app-state')['snapshot']['window']
            self.scroll_at(80, window['height'] * .4, direction)
        raise AssertionError((number, tree))
    def hotkey(self,key):return self.act('hotkey','--key',key,'--no-screenshot')
    def click(self,x,y,count=1):return self.act('click','--x',str(x),'--y',str(y),'--click-count',str(count),'--no-screenshot')

    def state(self):
        time.sleep(.2);row=next(csv.reader(self.input.open()))
        return dict(text=bytes.fromhex(row[1]).decode(),selected=list(map(int,row[2:4])),marked=list(map(int,row[4:6])),
            cursor=list(map(float,row[6:9])),undo=int(row[9]),redo=int(row[10]),composing=row[11]=='true',focused=row[12]=='true')

    def replace(self,text):
        assert self.state()['focused'],'No focused scratch text input'
        self.menu('編集', 'すべてを選択')
        if text and ('\n' in text or '\t' in text):
            # Keep our known scratch payload on the native clipboard until the
            # asynchronous GPUI Cmd+V handler has consumed it.
            subprocess.run(['pbcopy'],input=text.encode(),check=True)
            self.hotkey('CmdOrCtrl+V')
        elif text:self.act('type-text','--text',text,'--no-screenshot')
        else:self.key('Backspace')
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            state=self.state()
            if state['text']==text:return
            time.sleep(.1)
        raise AssertionError(state)

    def glyph_position(self,prefix,font=None):
        rows=list(csv.reader(self.trace.open()))
        row=next(r for r in reversed(rows) if len(r)==12 and r[4]=='0' and bytes.fromhex(r[3]).decode().startswith(prefix) and (font is None or float(r[2])==font))
        return float(row[6])+5,float(row[7])+32-5

    def edit(self,prefix,font=None):
        self.snapshot();x,y=self.glyph_position(prefix,font)
        helper = self.out / 'native-double-click'
        if not helper.exists():
            subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('native_double_click.swift')),
                            '-o', str(helper)], check=True)
        subprocess.run([str(helper), str(self.app.pid), str(x), str(y)], check=True)

    def save(self):self.press('保存');return self.fixture.read_bytes()

    def text(self,name=None):
        _,rows=self.snapshot(name)
        return ''.join(''.join(r['text'].split()) for r in rows)
