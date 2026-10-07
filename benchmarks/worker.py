"""Worker latency and sampled process-tree RSS; not a Ghostty frame benchmark."""
import json, os, pathlib, subprocess, tempfile, time, sys
root=pathlib.Path(__file__).resolve().parents[1]
worker=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else root/'worker/render.mjs'
delay=float(sys.argv[2]) if len(sys.argv)>2 else 0
for trial in range(3):
    with tempfile.TemporaryDirectory(prefix='termitex-bench-') as cache:
        env={**os.environ,'TFORMULA_CACHE_DIR':cache,'TERMITEX_TFORMULA':str(root/'node_modules/tformula')}
        start=time.perf_counter()
        p=subprocess.Popen(['node',str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=env)
        try:
            time.sleep(delay)
            elapsed=[]
            for key,latex in [('first',r'\nabla\times\mathbf{B}=\mu_0\mathbf{J}'),('new',r'\frac{1}{\sqrt{\mu_0\epsilon_0}}'),('cached',r'\nabla\times\mathbf{B}=\mu_0\mathbf{J}')]:
                req=dict(key=key,cell_width=16,cell_height=34,formula=dict(latex=latex,row=0,col=0,rows=1,cols=60,display=False,fg='#ffffff',bg='#282c34'))
                t=time.perf_counter();p.stdin.write(json.dumps(req)+'\n');p.stdin.flush()
                r=json.loads(p.stdout.readline());assert 'error' not in r,r
                elapsed.append(round((time.perf_counter()-t)*1000,2))
                if key=='first': first_total=round((time.perf_counter()-start)*1000,2)
            rows=[list(map(int,line.split())) for line in subprocess.check_output(['ps','-axo','pid=,ppid=,rss='],text=True).splitlines()]
            ids={p.pid}
            while True:
                more={pid for pid,ppid,rss in rows if ppid in ids}
                if more<=ids:break
                ids|=more
            rss=sum(rss for pid,ppid,rss in rows if pid in ids)/1024
            print(json.dumps(dict(trial=trial,delay_s=delay,request_ms=elapsed,launch_to_first_ms=first_total,processes=len(ids),sampled_rss_mib=round(rss,1))),flush=True)
        finally:
            p.stdin.close();p.wait(timeout=10)
