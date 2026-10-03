import argparse,csv,json,pathlib,shutil,subprocess,time
root=pathlib.Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser(description='6枚サンプルのmacOS全画面受け入れ確認')
parser.add_argument('--output',type=pathlib.Path,default=root/'target/mvp-acceptance')
parser.add_argument('--orca',default='orca')
parser.add_argument('--record',action='store_true')
args=parser.parse_args()
out=args.output
out.mkdir(parents=True,exist_ok=True)
ocr=root/'target/mvp-recognize-text'
subprocess.run(['xcrun','swiftc',str(pathlib.Path(__file__).with_name('recognize_text.swift')),'-o',str(ocr)],check=True)
headings=['CIのビルドが遅い','キャッシュの境界を見直す','依存と成果物を分ける','計測方法','ビルド時間を38%短縮','待ち時間を減らし、変更を速く届ける']
def action(*command):
    p=subprocess.run([args.orca,'computer',*command,'--app','psycho','--json'],capture_output=True,text=True)
    data=json.loads(p.stdout)
    if not data.get('ok'):
        if command[0]=='click' and data.get('error',{}).get('code')=='window_not_focused': return {'delivered_but_unverified':data}
        raise RuntimeError(data)
    time.sleep(.15)
    return data['result']
def observe(cycle,step,index,fullscreen):
    time.sleep(.2)
    state=action('get-app-state')
    file=out/f'fullscreen-{cycle:02}-{step}.png'
    shutil.copyfile(state['screenshot']['path'],file)
    text=subprocess.check_output([str(ocr),str(file)],text=True)
    normalized=''.join(text.split())
    assert headings[index] in normalized,(cycle,step,headings[index],text)
    window=state['snapshot']['window']
    assert window['width']==(1920 if fullscreen else 1280),(cycle,step,window)
    writer.writerow([cycle,step,index+1,window['width'],window['height'],'pass'])
    log.flush()
    return state
state=action('get-app-state')
if state['snapshot']['window']['width']==1920:
    action('press-key','--key','Escape','--no-screenshot')
    time.sleep(1)
with (out/'fullscreen-checks.csv').open('w',newline='') as log:
    writer=csv.writer(log);writer.writerow(['cycle','step','slide','width','height','result'])
    for cycle in range(1,11):
        action('click','--x','65','--y','222','--no-screenshot')
        action('click','--x',str(630 if cycle%2 else 730),'--y','54','--no-screenshot')
        time.sleep(.8)
        index=0 if cycle%2 else 2
        observe(cycle,'start',index,True)
        if cycle==1 and args.record:
            window=action('get-app-state')['snapshot']['window']['id']
            (out/'fullscreen.mov').unlink(missing_ok=True)
            subprocess.Popen(['screencapture','-v','-l',str(window),'-V','20','-x',str(out/'fullscreen.mov')])
        for key in ['Right','Down','Space','PageDown','Right','Down']:
            action('press-key','--key',key,'--no-screenshot')
            index=min(index+1,5)
            observe(cycle,'next-'+key+'-'+str(index),index,True)
        for key in ['Left','Up','PageUp','Left','Up','PageUp','Left']:
            action('press-key','--key',key,'--no-screenshot')
            index=max(index-1,0)
            observe(cycle,'previous-'+key+'-'+str(index),index,True)
        action('press-key','--key','Escape','--no-screenshot')
        time.sleep(.8)
        observe(cycle,'return',index,False)
        action('press-key','--key','Space','--no-screenshot')
        observe(cycle,'editor-focus',index,False)
        print(f'PASS fullscreen cycle {cycle}',flush=True)
