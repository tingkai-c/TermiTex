// TFormula 0.3.1 renderer reused under MIT; see TFORMULA-LICENSE.
// Rust owns layout. Native terminal prose is handled in Rust; no prose enters MathJax.
import readline from 'node:readline';
import {pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {dirname} from 'node:path';
const root=process.env.TERMITEX_TFORMULA || dirname(createRequire(import.meta.url).resolve('tformula/package.json'));
const {MathRenderer}=await import(pathToFileURL(`${root}/dist/math-renderer.js`));
const {calculateFormulaGeometry}=await import(pathToFileURL(`${root}/dist/geometry.js`));
const sharp=createRequire(pathToFileURL(`${root}/package.json`))('sharp');
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
  let png=result.png, columns=f.cols;
  if(!f.display && f.rows===1) {
   const geometry=calculateFormulaGeometry({aspectRatio:result.naturalAspectRatio,naturalHeightEx:result.naturalHeightEx,depthEx:0,
    columns:f.cols,rows:1,cell:caps.cell,scale:1,display:false,leftAlign:true});
   // Crop only unused trailing canvas. Keep the original baseline and bearing.
   columns=Math.min(f.cols,Math.max(1,Math.ceil((geometry.offsetX+geometry.formulaWidth)/req.cell_width)));
   png=await sharp(result.png).extract({left:0,top:0,width:columns*req.cell_width,height:req.cell_height}).png().toBuffer();
  }
  process.stdout.write(JSON.stringify({key:req.key,png:Buffer.from(png).toString('base64'),columns})+'\n');
 }catch(error){process.stdout.write(JSON.stringify({key:req?.key??'',error:String(error.message)})+'\n');}
}
process.exit();
