"""Native backend gate: real PNGs, sizing, representative syntax, and recovery.
Requires Pillow for decoded-pixel checks; no Node.js is used.
"""
import base64,copy,io,json,os,pathlib,struct,subprocess,tempfile
from PIL import Image,ImageChops,ImageDraw
root=pathlib.Path(__file__).resolve().parents[1]
fixtures=json.loads((root/'tests/fixtures/render.json').read_text())
requests=[f['request'] for f in fixtures]
for latex in [
 r'\nabla\cdot\mathbf{E}=\frac{\rho}{\epsilon_0}',
 r'\nabla\cdot\mathbf{B}=0',
 r'\nabla\times\mathbf{E}=-\frac{\partial\mathbf{B}}{\partial t}',
 r'\nabla\times\mathbf{B}=\mu_0\mathbf{J}+\mu_0\epsilon_0\frac{\partial\mathbf{E}}{\partial t}',
 r'c=\frac{1}{\sqrt{\mu_0\epsilon_0}}\approx3.00\times10^8\,\mathrm{m/s}',
 r'\int_0^1x^2\,dx=\frac{1}{3}',r'\sum_{k=1}^n k=\frac{n(n+1)}{2}',
 r'\frac{-b\pm\sqrt{b^2-4ac}}{2a}',r'P(A\mid B)=\frac{P(A\cap B)}{P(B)}',
 r'\begin{aligned}a&=b+c\\d&=e-f\end{aligned}',
 r'f(x)=\begin{cases}x^2&x\ge0\\-x&x<0\end{cases}',
 r'\left(\frac{a}{b}\right)^2',r'\operatorname{Var}(X)=\mathbb{E}[X^2]-\mathbb{E}[X]^2',
 r'\ce{H2SO4 + 2NaOH -> Na2SO4 + 2H2O}',r'\pu{1.5e-3 mol//L}',
 r'\hat{x}+\vec{v}+\overline{AB}',r'\boldsymbol{\alpha}+\mathcal{L}+\mathbb{R}',
 r'\newcommand{\foo}[1]{#1^2}\foo{x}',r'\text{中文}+x',
]:
 r=copy.deepcopy(requests[0]);r['formula'].update(latex=latex,display=True,rows=4,cols=100);requests.append(r)
# Repeated metrics/resize paths must produce correctly sized images.
for cw,ch in [(8,17),(12,24),(20,42)]:
 r=copy.deepcopy(requests[2]);r.update(cell_width=cw,cell_height=ch);requests.append(r)
for i,r in enumerate(requests):
 r['key']=str(i);r['formula'].setdefault('row',0);r['formula'].setdefault('col',0)
invalid=[]
for latex in [r'\notarealcommand',r'\frac{','{'*40+'x'+'}'*40,'x'*20001]:
 r=copy.deepcopy(requests[0]);r['formula']['latex']=latex;invalid.append(r)
# Known MathJax physics-extension gap: must fail cleanly instead of painting nonsense.
r=copy.deepcopy(requests[0]);r['formula']['latex']=r'\dv{f}{x}';invalid.append(r)
all_requests=requests+invalid+[requests[0]]
env={**os.environ,'PATH':'/usr/bin:/bin'}
result=subprocess.run([str(root/'target/release/termitex'),'--internal-ratex-worker'],input=''.join(json.dumps(r)+'\n' for r in all_requests),capture_output=True,text=True,timeout=30,check=True,env=env)
responses=[json.loads(line) for line in result.stdout.splitlines()];assert len(responses)==len(all_requests)
panels=[]
for r,response in zip(requests,responses):
 assert not response.get('error'),(r['formula']['latex'],response)
 png=base64.b64decode(response['png']);image=Image.open(io.BytesIO(png)).convert('RGB')
 assert image.size==(response['columns']*r['cell_width'],r['formula']['rows']*r['cell_height'])
 assert 0<response['columns']<=r['formula']['cols']
 if r['formula']['display']:assert response['columns']==r['formula']['cols']
 bg=Image.new('RGB',image.size,r['formula']['bg']);bounds=ImageChops.difference(image,bg).getbbox();assert bounds,r['formula']['latex']
 panels.append((r['formula']['latex'],image.crop(bounds)))
for response in responses[len(requests):-1]:assert response.get('error'),response
assert not responses[-1].get('error'),'worker did not recover after errors'
out=pathlib.Path(tempfile.mkdtemp(prefix='termitex-native-qa-'))
sheet=Image.new('RGB',(1100,100*len(panels)),'#282c34');draw=ImageDraw.Draw(sheet)
for i,(label,panel) in enumerate(panels):
 draw.text((8,i*100+3),label[:140],fill='#aaaaaa');sheet.paste(panel,(12,i*100+26))
sheet.save(out/'equations.png')
print(f'Native: {len(requests)} renders passed; {len(invalid)} invalid/unsupported inputs rejected; recovery passed; no Node on PATH')
print(f'Visual QA: {out}/equations.png')
