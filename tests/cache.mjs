import assert from 'node:assert/strict';
import {mkdtemp,rm,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {RenderCache,cacheKey} from '../worker/cache.mjs';
const root=await mkdtemp(join(tmpdir(),'termitex-cache-test-'));
try {
 let creates=0;const key=cacheKey('test'),create=async()=>{creates++;return Buffer.from('image');};
 let cache=new RenderCache({root});assert.equal((await cache.get(key,create)).toString(),'image');
 await cache.get(key,create);assert.equal(creates,1);
 cache=new RenderCache({root});await cache.get(key,create);assert.equal(creates,1);
 await writeFile(join(root,`${key}.cache`),'corrupt');
 cache=new RenderCache({root});await cache.get(key,create);assert.equal(creates,2);
 const blocked=join(root,'file');await writeFile(blocked,'not a directory');
 cache=new RenderCache({root:blocked});assert.equal((await cache.get(key,create)).toString(),'image');
 cache=new RenderCache({root,memoryBytes:3,diskBytes:3});await cache.get(cacheKey('other'),create);assert.equal(cache.bytes,0);
 console.log('Cache: memory/disk hits, corrupt entry repair, unwritable path fallback, memory budget');
}finally{await rm(root,{recursive:true,force:true});}
