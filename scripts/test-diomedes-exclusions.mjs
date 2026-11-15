import assert from 'node:assert/strict';
import { isDiomedesExcludedMod, modHasRunModeAffinity, isCullFactoryBlockedByReplay } from '../src/pages/launcher/mods.js';
for (const [dev,name] of [['HQHQTeam','VLog'],['HQHQTeam','HQoL'],['asta','LCF'],['Pooble','LCBetterSaves'],['timewaste','OverworkedUI'],['asta','Replay'],['mikuoreo','LCStatsTracker']]) {
  assert(isDiomedesExcludedMod({dev,name}));
}
assert(isDiomedesExcludedMod({dev:'New',name:'SaveUI',switch_group:'SAVE-UI'}));
for (const name of ['QuotaTools','FontPatcher','Replay']) assert(!isDiomedesExcludedMod({dev:'HQ',name}));
assert(modHasRunModeAffinity({dev:'AsTa',name:'LcF',tags:[]}));
assert(!isCullFactoryBlockedByReplay({dev:'fumiko',name:'CullFactory'},[{dev:'asta',name:'Replay'}],new Set(),81,'diomedes'));
console.log('Selective Diomedes exclusions and hidden LCF affinity passed.');
