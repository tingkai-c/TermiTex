// Pinned PNG regression fixtures captured from the prior pixel-verified backend.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {render} from '../worker/backend.mjs';
const fixtures=JSON.parse(await readFile(new URL('./fixtures/render.json',import.meta.url)));
for(const f of fixtures){
 const result=await render(f.request);
 assert.equal(result.columns,f.columns);
 assert.equal(createHash('sha256').update(Buffer.from(result.png,'base64')).digest('hex'),f.sha256,f.request.formula.latex);
}
await assert.rejects(render({key:'oversize',formula:{latex:'x',cols:5000,rows:1,display:false},cell_width:16,cell_height:34}),/raster limit/);
const request=fixtures[0].request;
for(const latex of [String.raw`\notarealcommand`,String.raw`\frac{`,String.raw`\input{file}`]){
 await assert.rejects(render({...request,formula:{...request.formula,cols:80,latex}}));
}
console.log('9 reference images match exactly; invalid formulas and oversized rasters rejected');
