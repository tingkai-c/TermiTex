// Nested-SVG adaptation derived from TFormula 0.3.1 (MIT); see TFORMULA-LICENSE.
// Runtime dependencies are MathJax and resvg only.
import {renderAsync} from '@resvg/resvg-js';
import {typeset,initialize} from './mathjax.mjs';
import {readSvgDimensions} from './svg.mjs';
import {calculateFormulaGeometry} from './geometry.mjs';
import {RenderCache,cacheKey} from './cache.mjs';
const cache=new RenderCache();
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
 const caps={cell:{width:req.cell_width,height:req.cell_height}};
 const containerWidth=f.display&&f.rows>1?Math.max(1,Math.round(Math.max(1,width-req.cell_width*2)*8/(req.cell_height*0.45))):100000;
 const svg=await typeset(f.latex,f.display&&f.rows>1,containerWidth,cache);
 const dimensions=readSvgDimensions(svg);
 const geometry=calculateFormulaGeometry({aspectRatio:dimensions.aspectRatio,naturalHeightEx:dimensions.heightEx,depthEx:dimensions.depthEx,
  columns:f.cols,rows:f.rows,cell:caps.cell,scale:1,display:f.display,leftAlign:!f.display&&!req.compatibility});
 if(geometry.fitScale<0.4)throw Error('formula needs more vertical space');
 const columns=!req.compatibility&&!f.display&&f.rows===1?Math.min(f.cols,Math.max(1,Math.ceil((geometry.offsetX+geometry.formulaWidth)/req.cell_width))):f.cols;
 const canvas=`<svg xmlns="http://www.w3.org/2000/svg" width="${columns*req.cell_width}" height="${height}"><rect width="100%" height="100%" fill="${escape(f.bg)}"/><g color="${escape(f.fg)}" fill="${escape(f.fg)}">${nested(svg,geometry)}</g></svg>`;
 const key=cacheKey({termitex:'resvg-2.6.2-single-pass-v2',canvas});
 const png=await cache.get(key,async()=>{
  const result=await renderAsync(canvas,{fitTo:{mode:'original'},font:{loadSystemFonts:/<(?:[A-Za-z_][\w.-]*:)?text(?=[\s/>])/iu.test(svg)},shapeRendering:2,textRendering:2,logLevel:'error'});
  const png=result.asPng();
  if(png.byteLength>12*1024*1024)throw Error('formula PNG exceeds size limit');
  return png;
 });
 return {key:req.key,png:png.toString('base64'),columns};
}
export async function prewarm() {
 // Initialize independently of the disk cache before the first equation arrives.
 await initialize();
 // Explicitly initialize native rasterization even on a persistent cache hit.
 await renderAsync('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path d="M0 0h1v1H0z"/></svg>',{font:{loadSystemFonts:false}});
}
