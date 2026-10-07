"""Compare fresh worker processes on 81 distinct equations; outputs raw measurements."""
import base64,copy,json,os,pathlib,select,statistics,struct,subprocess,sys,tempfile,time
output_dir=pathlib.Path(tempfile.mkdtemp(prefix="termitex-ratex-images-"))
root=pathlib.Path(__file__).resolve().parents[2]
fixtures=json.loads((root/'tests/fixtures/render.json').read_text())
# Put a real Maxwell equation first, then exercise every fixture.
fixtures=[fixtures[6]]+fixtures[:6]+fixtures[7:]
requests=[]
for i in range(9):
    for fixture in fixtures:
        r=copy.deepcopy(fixture['request']);r['key']=str(len(requests))
        if i:r['formula']['latex']+=' + '+str(i)
        requests.append(r)

def run(backend,command,targets):
    with tempfile.TemporaryDirectory(prefix='termitex-compare-') as cache:
        env={**os.environ,'TERMITEX_CACHE_DIR':cache}
        started=time.perf_counter()
        p=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
        latencies=[];sizes=[];rss=[];mismatches=[]
        try:
            for index,req in enumerate(requests):
                r=copy.deepcopy(req)
                if targets:r.update(target_width=targets[index][0],target_height=targets[index][1])
                t=time.perf_counter();p.stdin.write(json.dumps(r)+'\n');p.stdin.flush()
                if not select.select([p.stdout],[],[],30)[0]:raise TimeoutError(backend)
                line=p.stdout.readline()
                if not line:raise RuntimeError(p.stderr.read())
                response=json.loads(line)
                if 'error' in response:raise RuntimeError((r,response))
                png=base64.b64decode(response['png']);assert png[:8]==b'\x89PNG\r\n\x1a\n'
                sizes.append(struct.unpack('>II',png[16:24]))
                if index<9:(output_dir/f'{backend}-{index}.png').write_bytes(png)
                if targets and sizes[-1]!=tuple(targets[index]):mismatches.append([index,targets[index],sizes[-1]])
                latencies.append((time.perf_counter()-t)*1000)
                if index==0:first_ms=(time.perf_counter()-started)*1000
                if index in [8,len(requests)-1]:rss.append(int(subprocess.check_output(['ps','-o','rss=','-p',str(p.pid)],text=True))/1024)
            p.stdin.close()
            pid,status,usage=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status)
            assert p.returncode==0,p.stderr.read()
            return dict(backend=backend,count=len(requests),first_ms=first_ms,warm_median_ms=statistics.median(latencies[9:]),warm_p95_ms=sorted(latencies[9:])[int(.95*len(latencies[9:]))],cpu_ms=(usage.ru_utime+usage.ru_stime)*1000,peak_rss_mib=usage.ru_maxrss/(1024**2 if sys.platform=='darwin' else 1024),rss_after_9_81_mib=rss,size_mismatches=mismatches),sizes
        finally:
            if p.returncode is None:p.kill();p.wait()

native=[sys.argv[1]]
js=['node',str(root/'worker/render.mjs')]
results=[];targets=None
for trial in range(5):
    order=[('mathjax',js),('ratex',native)] if trial%2==0 else [('ratex',native),('mathjax',js)]
    for name,command in order:
        row,sizes=run(name,command,targets if name=='ratex' else None)
        if name=='mathjax':targets=sizes
        row['trial']=trial;results.append(row);print(json.dumps(row),flush=True)

print(f"Sample images: {output_dir}",file=sys.stderr)
