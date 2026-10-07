// TFormula 0.3.1 renderer reused under MIT; see TFORMULA-LICENSE.
// Rust owns layout. No inline tail composition and no prose enters MathJax.
import readline from 'node:readline';
import {pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {dirname} from 'node:path';
const root=process.env.TERMITEX_TFORMULA || dirname(createRequire(import.meta.url).resolve('tformula/package.json'));
const {MathRenderer}=await import(pathToFileURL(`${root}/dist/math-renderer.js`));
const renderer=new MathRenderer();
for await(const line of readline.createInterface({input:process.stdin,crlfDelay:Infinity})) {
 let req;
 try {
  req=JSON.parse(line); const f=req.formula;
  if(f.latex.length>20000)throw Error('formula too long');
  const rect={startRow:0,endRow:f.rows-1,startCol:0,endCol:f.cols};
  const plan={formula:{source:rect,latex:f.latex,intent:f.display?'display':'inline',confidence:'explicit'},canvas:rect,
   sourceMasks:Array.from({length:f.rows},(_,rowOffset)=>({rowOffset,startCol:0,endCol:f.cols,logicalStartCol:0})),formulaSlices:[],mode:'source',estimatedQuality:1,composite:false};
  const caps={kittyGraphics:true,cell:{width:req.cell_width,height:req.cell_height,source:'override'},foreground:f.fg,background:f.bg};
  const result=await renderer.renderPlacement(plan,caps,1);
  if(result.fitScale<0.4)throw Error('formula needs more vertical space');
  process.stdout.write(JSON.stringify({key:req.key,png:Buffer.from(result.png).toString('base64')})+'\n');
 }catch(error){process.stdout.write(JSON.stringify({key:req?.key??'',error:String(error.message)})+'\n');}
}
process.exit();
