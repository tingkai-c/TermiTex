import readline from 'node:readline';
import {render,prewarm} from './backend.mjs';
// All initialization happens in the worker; the Rust PTY loop remains live.
// Prewarming is optional for benchmarking and failures never disable rendering.
if(process.env.TERMITEX_PREWARM!=='0') {
 try {await prewarm();} catch(error) {
  if(process.env.TERMITEX_DEBUG)process.stderr.write(`prewarm: ${error.message}\n`);
 }
}
for await(const line of readline.createInterface({input:process.stdin,crlfDelay:Infinity})) {
 let req;
 try {req=JSON.parse(line);process.stdout.write(JSON.stringify(await render(req))+'\n');}
 catch(error){process.stdout.write(JSON.stringify({key:req?.key??'',error:String(error.message)})+'\n');}
}
