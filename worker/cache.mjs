import {createHash,randomUUID} from 'node:crypto';
import {mkdir,readFile,writeFile,rename,unlink,readdir,stat} from 'node:fs/promises';
import {homedir} from 'node:os';
import {join} from 'node:path';
export const cacheKey=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
const digest=data=>createHash('sha256').update(data).digest();
export class RenderCache {
 constructor({root=process.env.TERMITEX_CACHE_DIR || join(process.platform==='darwin'?join(homedir(),'Library','Caches'):process.env.XDG_CACHE_HOME||join(homedir(),'.cache'),'termitex','v1'),memoryBytes=16*1024*1024,diskBytes=128*1024*1024}={}) {
  this.root=root;this.limit=memoryBytes;this.diskLimit=diskBytes;this.memory=new Map();this.bytes=0;this.writes=0;
 }
 remember(key,data){
  const previous=this.memory.get(key);if(previous)this.bytes-=previous.length;
  this.memory.delete(key);this.memory.set(key,data);this.bytes+=data.length;
  while(this.bytes>this.limit||this.memory.size>128){const key=this.memory.keys().next().value;this.bytes-=this.memory.get(key).length;this.memory.delete(key);}
 }
 async get(key,create){
  if(!/^[a-f0-9]{64}$/.test(key))throw Error('invalid cache key');
  const hit=this.memory.get(key);if(hit){this.remember(key,hit);return hit;}
  const path=join(this.root,`${key}.cache`);
  try {
   const meta=await stat(path);
   if(meta.size<=12*1024*1024+32){const stored=await readFile(path),data=stored.subarray(32);if(stored.length>32&&digest(data).equals(stored.subarray(0,32))){this.remember(key,data);return data;}}
  }catch{/* Cache availability never controls rendering. */}
  const data=Buffer.from(await create());this.remember(key,data);
  if(data.length>12*1024*1024)return data;
  const temp=join(this.root,`${key}.${randomUUID()}.tmp`);
  try {
   await mkdir(this.root,{recursive:true,mode:0o700});
   await writeFile(temp,Buffer.concat([digest(data),data]),{flag:'wx',mode:0o600});
   await rename(temp,path);
   if(this.writes++%32===0)await this.prune();
  }catch{/* A read-only/full cache falls back to memory. */}
  finally{await unlink(temp).catch(()=>{});}
  return data;
 }
 async prune(){
  const names=(await readdir(this.root)).filter(name=>/^[a-f0-9]{64}\.cache$/.test(name));
  const files=(await Promise.all(names.map(async name=>{const path=join(this.root,name);try{return {path,...await stat(path)};}catch{return null;}}))).filter(Boolean).sort((a,b)=>a.mtimeMs-b.mtimeMs);
  let total=files.reduce((sum,f)=>sum+f.size,0),count=files.length;
  for(const f of files){if(total<=this.diskLimit&&count<=2048)break;await unlink(f.path).catch(()=>{});total-=f.size;count--;}
 }
}
