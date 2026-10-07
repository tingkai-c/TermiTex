// Pixel parity against the previous TFormula + Sharp pipeline.
import assert from 'node:assert/strict';
import sharp from 'sharp';
import {MathRenderer} from 'tformula/dist/math-renderer.js';
import {calculateFormulaGeometry} from 'tformula/dist/geometry.js';
import {closeRasterWorker} from 'tformula/dist/resvg-client.js';
import {render} from '../worker/backend.mjs';
const baseline=new MathRenderer();
try {
 for(const [latex,display,rows,cols] of [
  ['J',false,1,5],[String.raw`\rho`,false,1,8],[String.raw`\partial/\partial t`,false,1,12],
  [String.raw`e^{i\pi}+1=0`,false,1,24],[String.raw`\frac{1}{2}`,false,1,16],
  [String.raw`\displaystyle\frac{1}{2}`,true,3,80],
  [String.raw`\nabla\times\mathbf{B}=\mu_0\mathbf{J}`,true,3,80],
  [String.raw`\begin{pmatrix}a&b\\c&d\end{pmatrix}`,true,4,80],
  [String.raw`\ce{H2O}`,false,1,14],
 ]) {
  const f={latex,display,rows,cols,fg:'#f0e0d0',bg:'#282c34'};
  const rect={startRow:0,endRow:rows-1,startCol:0,endCol:cols};
  const caps={cell:{width:16,height:34},foreground:f.fg,background:f.bg};
  const plan={formula:{source:rect,latex,intent:display?'display':'inline',confidence:'explicit'},canvas:rect,sourceMasks:Array.from({length:rows},(_,rowOffset)=>({rowOffset,startCol:0,endCol:cols,logicalStartCol:0})),formulaSlices:[],mode:'source',estimatedQuality:1,composite:false};
  const old=await baseline.renderPlacement(plan,caps,1);
  const fresh=await render({key:'test',formula:f,cell_width:16,cell_height:34});
  let expected=sharp(old.png);
  if(!display){const g=calculateFormulaGeometry({aspectRatio:old.naturalAspectRatio,naturalHeightEx:old.naturalHeightEx,columns:cols,rows:1,cell:caps.cell,scale:1,display:false,leftAlign:true});const w=Math.min(cols,Math.max(1,Math.ceil((g.offsetX+g.formulaWidth)/16)));assert.equal(fresh.columns,w);expected=expected.extract({left:0,top:0,width:w*16,height:34});}
  assert.deepEqual(await sharp(Buffer.from(fresh.png,'base64')).ensureAlpha().raw().toBuffer(),await expected.ensureAlpha().raw().toBuffer(),latex);
 }
 await assert.rejects(render({key:'oversize',formula:{latex:'x',cols:5000,rows:1,display:false},cell_width:16,cell_height:34}),/raster limit/);
 console.log('9 representative formulas: identical decoded pixels; raster limit enforced');
} finally {closeRasterWorker();}
