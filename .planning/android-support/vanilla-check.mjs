import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { writeFile } from 'node:fs/promises';
const exec = promisify(execFile);
const adb = async (...args) => (await exec('C:/Users/pAsta/AppData/Local/Android/Sdk/platform-tools/adb.exe', ['-s', 'R5KL209P1HY', ...args])).stdout;
const path = 'files/game-environment.json';
const backup = `${path}.hq-vanilla-check`;
if (process.argv.includes('--restore')) {
  await adb('shell', 'run-as', 'asta.hq_launcher', 'cp', backup, path);
  await adb('shell', 'run-as', 'asta.hq_launcher', 'rm', backup);
  console.log('Original game environment restored.');
} else {
  const original = await adb('shell', 'run-as', 'asta.hq_launcher', 'cat', path);
  const config = JSON.parse(original);
  if (config.version !== 1 || !config.games?.['1966720']) throw new Error('Unexpected game environment');
  try { await adb('shell', 'run-as', 'asta.hq_launcher', 'ls', backup); throw new Error('Diagnostic backup already exists'); }
  catch (error) { if (!error.stderr?.includes('No such file')) throw error; }
  const game = config.games['1966720'];
  const previous = game.WINEDLLOVERRIDES;
  if (typeof previous !== 'string' || !previous.split(';').includes('winhttp=n,b')) throw new Error('Unexpected winhttp configuration');
  game.WINEDLLOVERRIDES = previous.split(';').map(value => value === 'winhttp=n,b' ? 'winhttp=b' : value).join(';');
  await adb('shell', 'run-as', 'asta.hq_launcher', 'cp', path, backup);
  await writeFile('C:/Users/pAsta/hq-launcher/.planning/android-support/vanilla-environment.json', JSON.stringify(config));
  await adb('push', 'C:/Users/pAsta/hq-launcher/.planning/android-support/vanilla-environment.json', '/data/local/tmp/hq-vanilla-environment.json');
  await adb('shell', 'run-as', 'asta.hq_launcher', 'cp', '/data/local/tmp/hq-vanilla-environment.json', path);
  await adb('shell', 'rm', '/data/local/tmp/hq-vanilla-environment.json');
  console.log('For this diagnostic, Wine uses built-in winhttp. Mod files and saves unchanged.');
}
