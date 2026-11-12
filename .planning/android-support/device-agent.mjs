import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile, writeFile} from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
const exec=promisify(execFile);
const adb=async(...args)=>(await exec('C:/Users/pAsta/AppData/Local/Android/Sdk/platform-tools/adb.exe',['-s','R5KL209P1HY',...args],{maxBuffer:2*1024*1024})).stdout.trim();
const request=JSON.parse(await readFile(process.argv[2],'utf8'));
const id=randomUUID();
const local=`.planning/android-support/request-${id}.json`, temp=`/data/local/tmp/hq-request-${id}.json`;
const remote=`files/session/agent/req/${id}.json`, response=`files/session/agent/resp/${id}.json`;
await writeFile(local,JSON.stringify(request));
await adb('push',local,temp);
try {
  await adb('shell','run-as','asta.hq_launcher','cp',temp,`${remote}.tmp`);
  await adb('shell','run-as','asta.hq_launcher','mv',`${remote}.tmp`,remote);
  const deadline=Date.now()+Math.min(45000,((request.timeout||15)+3)*1000);
  let result;
  while(Date.now()<deadline){
    try {result=JSON.parse(await adb('shell','run-as','asta.hq_launcher','cat',response));break;}catch{}
    await new Promise(resolve=>setTimeout(resolve,150));
  }
  if(!result)throw Error('Guest request timed out');
  console.log(JSON.stringify(result));
  if(!result.ok)process.exitCode=1;
} finally {
  await adb('shell','rm','-f',temp);
  await adb('shell','run-as','asta.hq_launcher','rm','-f',remote,`${remote}.tmp`,response);
}
