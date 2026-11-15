// Exercise the actual launcher page with controlled IPC, without installing any game files.
async (page) => {
  const tab = await page.context().newPage();
  const errors = [];
  tab.on('pageerror', error => errors.push(error.message));
  try {
    await tab.route(/\/src\/main\.jsx(?:\?|$)/, route => route.fulfill({ contentType: 'text/javascript', body: `
      import React from '/node_modules/.vite/deps/react.js';
      import ReactDOM from '/node_modules/.vite/deps/react-dom_client.js';
      import LauncherPage from '/src/pages/LauncherPage.jsx';
      import '/src/index.css';
      ReactDOM.createRoot(document.getElementById('root')).render(React.createElement(LauncherPage, {}));
    ` }));
    await tab.addInitScript(() => {
      localStorage.clear();
      localStorage.setItem('selectedRunMode', 'diomedes');
      window.__presetCalls = [];
      const hq = { dev: 'HQ', name: 'QuotaTools', enabled: true, tags: [] };
      const diomedes = { dev: 'DiomedesLC', name: 'Diomedes', enabled: true, tags: ['Diomedes'] };
      const lcf = { dev: 'asta', name: 'LCF', enabled: true, tags: [] };
      const blocked = [['asta','Replay'],['mikuoreo','LCStatsTracker'],['HQHQTeam','HQoL'],['HQHQTeam','VLog'],['Pooble','LCBetterSaves']].map(([dev,name])=>({dev,name,enabled:true,tags:[]}));
      let id = 0;
      const callbacks = new Map();
      const listeners = new Map();
      const emit = (event, payload) => callbacks.get(listeners.get(event))?.({event, payload});
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
      window.__TAURI_INTERNALS__ = {
        transformCallback: cb => { callbacks.set(++id, cb); return id; },
        unregisterCallback: key => callbacks.delete(key), convertFileSrc: path => path,
        invoke: async (command, args) => {
          if (command === 'plugin:event|listen') { listeners.set(args.event, args.handler); return args.handler; }
          if (command === 'get_selected_run_mode') return 'diomedes';
          if (command === 'get_selected_version') return 73;
          if (command === 'get_manifest') return { version: 1, mods: [hq, diomedes, lcf,...blocked], manifests: { 73: 'old', 81: 'latest' } };
          if (command === 'get_events') return { events: [] };
          if (command === 'get_events_enabled') return false;
          if (command === 'list_installed_versions') return [73, 81];
          if (command === 'get_game_status') return { running: false, pid: null };
          if (command === 'list_installed_mod_versions') return [hq, diomedes, lcf,...blocked].map(m => ({...m, version: '1.0.0'}));
          if (command === 'get_config_link_state_for_version') return { linked: true };
          if (command === 'get_lcstats_settings') return {};
          if (command === 'google_lcstats_auth_status') return { authenticated: false };
          if (command === 'prepare_preset' || command === 'set_selected_run_mode') window.__presetCalls.push({ command, args });
          if (command === 'launch_game_preset') throw new Error('Fixture launch failure');
          if (command === 'prepare_preset' && args.preset === 'diomedes' && window.__cancelNextPrepare) {
            window.__cancelNextPrepare = false;
            return new Promise((resolve, reject) => {
              window.__cancelPrepare = () => reject('Cancelled');
              emit('download://progress', {version: 81, step_name: 'Preset Mods', step_progress: 0, overall_percent: 0, extracted_files: 0, total_files: 35});
            });
          }
          if (command === 'cancel_prepare') { window.__cancelPrepare?.(); return true; }
          if (command === 'check_mod_updates' && args.runMode === 'diomedes' && window.__updatesEnabled) {
            emit('updatable://progress', {version:81, run_mode:'diomedes', total:1, checked:1, updatable_mods:['Leaf-Dependency']});
            emit('updatable://finished', {version:81, run_mode:'diomedes'});
          }
          if (command === 'apply_mod_updates') {
            window.__presetCalls.push({command,args});
            emit('download://progress', {version:81, step_name:'Update Mods', step_progress:0, overall_percent:0, total_files:1, extracted_files:0});
            if (window.__updateFailure) {
              emit('download://error', {version:81, run_mode:'diomedes', message:'Fixture update failure'});
              throw new Error('Fixture update failure');
            }
            emit('download://finished', {version:81, run_mode:'diomedes'});
          }
          if (command === 'get_disabled_mods' || command === 'get_practice_mod_list' || command.startsWith('list_')) return [];
          return null;
        }
      };
    });
    await tab.setViewportSize({width:1280,height:720});
    await tab.goto('http://127.0.0.1:1421');
    await tab.getByRole('button',{name:'HQ Run',exact:true}).waitFor();
    await tab.locator('[data-launcher-mod-row]').filter({hasText:'QuotaTools'}).waitFor();
    for (const name of ['LCF','Diomedes']) {
      if (await tab.locator('[data-launcher-mod-row]').filter({hasText:name}).count()) throw new Error(`${name} remains in the normal mod list`);
    }
    await tab.getByRole('button',{name:'Select run mode',exact:true}).click();
    if (await tab.getByRole('menuitem',{name:/LCF|Diomedes/}).count()) throw new Error('Retired mode remains selectable');
    if (await tab.evaluate(()=>localStorage.getItem('selectedRunMode')) !== 'hq') throw new Error('Diomedes preference was not migrated to HQ');
    if (errors.length) throw new Error(errors.join('\n'));
    return 'Retired Diomedes: local/backend selection restores HQ, both hidden modes absent from menu and mod list, base mods remain visible.';
  } finally { await tab.close(); }
}
