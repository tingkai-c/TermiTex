// Geometry and nested-SVG adaptation derived from TFormula 0.3.1 (MIT).
// See TFORMULA-LICENSE. MathJax compatibility remains provided by TFormula.
import {pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {dirname} from 'node:path';
const root=process.env.TERMITEX_TFORMULA || dirname(createRequire(import.meta.url).resolve('tformula/package.json'));
const moduleAt=name=>import(pathToFileURL(`${root}/dist/${name}.js`));
const {renderMathJaxSvg,renderMathJaxMathMl,readSvgDimensions,mathJaxContainerWidthForPlacement,mathRendererInternals}=await moduleAt('math-renderer');
const {calculateFormulaGeometry}=await moduleAt('geometry');
const {FormulaCache,formulaCacheKey}=await moduleAt('formula-cache');
const {mathRendererConfig}=await moduleAt('math-renderer-config');
const {renderAsync}=createRequire(pathToFileURL(`${root}/package.json`))('@resvg/resvg-js');
const cache=new FormulaCache({memoryEntries:128,maxMemoryBytes:16*1024*1024});
const escape=value=>value.replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
function nested(svg,g) {
 return svg.replace(/^<svg\b([^>]*)>/u,(_all,attrs)=>{
  const cleaned=attrs.replace(/\s(?:width|height|x|y|style)="[^"]*"/gu,'').replace(/\sxmlns="[^"]*"/u,'');
  return `<svg x="${g.offsetX}" y="${g.offsetY}" width="${g.formulaWidth}" height="${g.formulaHeight}"${cleaned}>`;
 });
}
export async function render(req) {
 const f=req.formula;
 if(f.latex.length>20000)throw Error('formula too long');
 const width=f.cols*req.cell_width,height=f.rows*req.cell_height;
 if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>4096||height>4096)throw Error('formula canvas exceeds raster limit');
 const rect={startRow:0,endRow:f.rows-1,startCol:0,endCol:f.cols};
 const plan={formula:{source:rect,latex:f.latex,intent:f.display?'display':'inline'},canvas:rect,sourceMasks:[],formulaSlices:[],mode:'source',composite:false};
 const caps={cell:{width:req.cell_width,height:req.cell_height}};
 const svg=await renderMathJaxSvg(f.latex,f.display,mathJaxContainerWidthForPlacement(plan,caps,1),cache);
 const dimensions=readSvgDimensions(svg);
 const geometry=calculateFormulaGeometry({aspectRatio:dimensions.aspectRatio,naturalHeightEx:dimensions.heightEx,depthEx:dimensions.depthEx,
  columns:f.cols,rows:f.rows,cell:caps.cell,scale:1,display:f.display,leftAlign:!f.display});
 if(geometry.fitScale<0.4)throw Error('formula needs more vertical space');
 const columns=!f.display&&f.rows===1?Math.min(f.cols,Math.max(1,Math.ceil((geometry.offsetX+geometry.formulaWidth)/req.cell_width))):f.cols;
 const canvas=`<svg xmlns="http://www.w3.org/2000/svg" width="${columns*req.cell_width}" height="${height}"><rect width="100%" height="100%" fill="${escape(f.bg)}"/><g color="${escape(f.fg)}" fill="${escape(f.fg)}">${nested(svg,geometry)}</g></svg>`;
 const key=formulaCacheKey({termitex:'single-pass-v1',canvas,config:mathRendererConfig.fingerprint});
 const png=await cache.getOrCreatePng(key,async()=>{
  const result=await renderAsync(canvas,{fitTo:{mode:'original'},font:{loadSystemFonts:mathRendererConfig.loadSystemFonts&&mathRendererInternals.svgNeedsSystemFonts(svg),...(mathRendererConfig.fontFiles?{fontFiles:mathRendererConfig.fontFiles}:{})},shapeRendering:2,textRendering:2,logLevel:'error'});
  const png=result.asPng();
  if(png.byteLength>12*1024*1024)throw Error('formula PNG exceeds size limit');
  return png;
 });
 return {key:req.key,png:png.toString('base64'),columns};
}
export async function prewarm() {
 // MathML forces initialization even if the representative SVG is disk-cached.
 await renderMathJaxMathMl(String.raw`\frac{\partial f}{\partial t}+\nabla\times\mathbf{B}+\mu_0+\epsilon_0+\int_0^1 x^2\,dx`,false);
 // Explicitly initialize native rasterization even on a persistent cache hit.
 await renderAsync('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path d="M0 0h1v1H0z"/></svg>',{font:{loadSystemFonts:false}});
}
