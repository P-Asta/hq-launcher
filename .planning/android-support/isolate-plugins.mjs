import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {writeFile} from 'node:fs/promises';
const exec=promisify(execFile);
const adb=async(...args)=>(await exec('C:/Users/pAsta/AppData/Local/Android/Sdk/platform-tools/adb.exe',['-s','R5KL209P1HY',...args])).stdout.trim();
const quote=value=>"'"+value.replaceAll("'", "'\\''")+"'";
const sh=command=>adb('shell',`run-as asta.hq_launcher sh -c ${quote(command)}`);
const base='files/linuxfs/root/.local/share/Steam/steamapps/common/Lethal Company';
const waitStopped=async()=>{for(let n=0;n<20;n++){const probe=JSON.parse((await exec(process.execPath,['.planning/android-support/device-check.mjs'])).stdout);if(!probe.runtime.session_running&&probe.runtime.session_phase==='idle')return;await new Promise(r=>setTimeout(r,500));}throw Error('Runtime did not stop');};
if(process.argv.includes('--isolate')) {
  await waitStopped();
  await sh(`cd ${quote(base)} && test ! -e .hq-isolation-plugins && mv BepInEx/plugins .hq-isolation-plugins && mkdir BepInEx/plugins`);
  console.log('Plugins preserved outside BepInEx for a loader-only diagnostic. Must restore before final.');
} else if(process.argv.includes('--group') || process.argv.includes('--restore')) {
  await waitStopped();
  const current=(await sh(`cd ${quote(base)} && ls BepInEx/plugins`)).split(/\r?\n/).filter(Boolean);
  if(current.some(name=>!/^[-A-Za-z0-9_]+$/.test(name)))throw Error('Unexpected plugin folder');
  for(const name of current)await sh(`cd ${quote(base)} && test ! -e ${quote('.hq-isolation-plugins/'+name)} && mv ${quote('BepInEx/plugins/'+name)} .hq-isolation-plugins/`);
  if(process.argv.includes('--restore')) {
    await sh(`cd ${quote(base)} && rmdir BepInEx/plugins && mv .hq-isolation-plugins BepInEx/plugins`);
    console.log('Original plugins restored.');
  } else {
    const wanted=JSON.parse(process.argv[process.argv.indexOf('--group')+1]);
    if(!Array.isArray(wanted)||wanted.some(name=>!/^[-A-Za-z0-9_]+$/.test(name)))throw Error('Invalid group');
    for(const name of wanted)await sh(`cd ${quote(base)} && mv ${quote('.hq-isolation-plugins/'+name)} BepInEx/plugins/`);
    console.log('Diagnostic group enabled: '+wanted.join(', '));
  }
} else throw Error('Choose --isolate or --restore');
