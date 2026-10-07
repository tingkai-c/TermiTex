import mathjax from '@mathjax/src';
import {SCIENTIFIC_TEX_PACKAGES,SCIENTIFIC_TEX_MACROS,normalizeLatexForRendering} from './tex-profile.mjs';
import {readSvgDimensions} from './svg.mjs';
import {cacheKey} from './cache.mjs';
let initialized;
export function initialize(){
 return initialized??=mathjax.init({
  loader:{load:['input/tex',...SCIENTIFIC_TEX_PACKAGES.filter(name=>name!=='configmacros').map(name=>`[tex]/${name}`),'output/svg']},
  tex:{maxBuffer:20000,maxMacros:1000,packages:{'[+]':SCIENTIFIC_TEX_PACKAGES},macros:SCIENTIFIC_TEX_MACROS},
  svg:{fontCache:'local',displayOverflow:'linebreak',linebreaks:{inline:false}},
 });
}
function validate(svg){
 if(!/^<svg\b/.test(svg)||!svg.includes('viewBox=')||!svg.includes('data-mml-node="math"'))throw Error('MathJax produced an incomplete SVG');
 if(/data-mjx-error=|data-mml-node="merror"|<merror\b/.test(svg)||/<g\b(?=[^>]*data-mml-node="mtext")(?=[^>]*fill="red")(?=[^>]*stroke="red")/.test(svg))throw Error('MathJax could not parse the formula');
 const d=readSvgDimensions(svg);if(!Number.isFinite(d.aspectRatio)||d.aspectRatio<=0)throw Error('invalid SVG dimensions');
}
export async function typeset(latex,display,containerWidth,cache){
 if(typeof latex!=='string'||latex.length>20000)throw Error('formula too long or invalid');
 const source=normalizeLatexForRendering(latex);
 if(/\\(?:require|href|url|html|class|cssId|style|includegraphics|input|include|usepackage|documentclass)\b/iu.test(source))throw Error('formula contains a disabled command');
 const key=cacheKey({svg:'termitex-mathjax-4.1.3-v1',source,display,containerWidth,packages:SCIENTIFIC_TEX_PACKAGES,macros:SCIENTIFIC_TEX_MACROS});
 const data=await cache.get(key,async()=>{
  await initialize();
  const node=await mathjax.tex2svgPromise(source,{display,em:16,ex:8,containerWidth});
  const adaptor=mathjax.startup.adaptor,svgNode=adaptor.tags(node,'svg')[0];
  if(!svgNode)throw Error('MathJax returned no SVG');
  const svg=adaptor.serializeXML(svgNode);validate(svg);return Buffer.from(svg);
 });
 const svg=data.toString('utf8');validate(svg);return svg;
}
