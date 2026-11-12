import {execFile} from 'node:child_process'; import {promisify} from 'node:util'; import {writeFile} from 'node:fs/promises';
const exec=promisify(execFile);const adb=async(...a)=>(await exec('C:/Users/pAsta/AppData/Local/Android/Sdk/platform-tools/adb.exe',['-s','R5KL209P1HY',...a])).stdout;
const path='files/linuxfs/root/.local/share/Steam/steamapps/common/Lethal Company/BepInEx/config/Zaggy1024.PathfindingLib.cfg';
const q=v=>"'"+v.replaceAll("'","'\\''")+"'"; const cmd=c=>adb('shell',`run-as asta.hq_launcher sh -c ${q(c)}`);
if(process.argv.includes('--restore')){await cmd('cp '+q(path+'.hq-check')+' '+q(path)+' && rm '+q(path+'.hq-check'));console.log('Original PathfindingLib config restored.');process.exit(0);}
const text=await cmd('cat '+q(path));if(!text.includes('PatchOffMeshConnectionStutterStepping = true'))throw Error('Unexpected config');
await cmd('test ! -e '+q(path+'.hq-check')+' && cp '+q(path)+' '+q(path+'.hq-check'));
await writeFile('.planning/android-support/pathfinding-config-false.cfg',text.replace('PatchOffMeshConnectionStutterStepping = true','PatchOffMeshConnectionStutterStepping = false'));
await adb('push','.planning/android-support/pathfinding-config-false.cfg','/data/local/tmp/hq-pathfinding-config.cfg');await cmd('cp /data/local/tmp/hq-pathfinding-config.cfg '+q(path));await adb('shell','rm','/data/local/tmp/hq-pathfinding-config.cfg');console.log('Optional native hook disabled for one diagnostic; original config backup retained.');
