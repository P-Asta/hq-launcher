// Render the actual desktop page with an IPC fixture, independently of AppRoot/login.
async (page) => {
  const testPage = await page.context().newPage();
  const errors = [];
  testPage.on('pageerror', error => errors.push(error.message));
  try {
    await testPage.route(/\/src\/main\.jsx(?:\?|$)/, route => route.fulfill({ contentType: 'text/javascript', body: `
      import React from '/node_modules/.vite/deps/react.js';
      import ReactDOM from '/node_modules/.vite/deps/react-dom_client.js';
      import LauncherPage from '/src/pages/LauncherPage.jsx';
      import '/src/index.css';
      ReactDOM.createRoot(document.getElementById('root')).render(React.createElement(LauncherPage, {}));
    ` }));
    await testPage.addInitScript(() => {
      localStorage.clear();
      const mod = { dev: 'HQ', name: 'QuotaTools', version: '1.0.0', enabled: true, tags: [], description: 'Quota settings' };
      const callbacks = new Map();
      let id = 0;
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
      window.__TAURI_INTERNALS__ = {
        transformCallback: callback => { callbacks.set(++id, callback); return id; },
        unregisterCallback: key => callbacks.delete(key),
        convertFileSrc: path => path,
        invoke: async command => {
          if (command === 'plugin:event|listen') return ++id;
          if (command === 'plugin:event|unlisten') return null;
          if (command === 'get_selected_run_mode') return 'hq';
          if (command === 'get_selected_version') return 81;
          if (command === 'get_manifest') return { version: 1, mods: [mod], manifests: { 81: 'fixture' } };
          if (command === 'get_events') return { version: 0, events: [] };
          if (command === 'get_events_enabled') return false;
          if (command === 'list_installed_versions') return [81];
          if (command === 'get_game_status') return { running: false, pid: null };
          if (command === 'list_installed_mod_versions') return [mod];
          if (command === 'get_lcstats_settings') return {};
          if (command === 'google_lcstats_auth_status') return { authenticated: false };
          if (command === 'get_config_link_state_for_version') return { linked: true };
          if (command === 'get_disabled_mods' || command === 'get_practice_mod_list' || command.startsWith('list_')) return [];
          return null;
        },
      };
    });
    await testPage.setViewportSize({ width: 1280, height: 720 });
    await testPage.goto('http://127.0.0.1:1421');
    const modRow = testPage.locator('[data-launcher-mod-row]').filter({ hasText: 'QuotaTools' });
    await modRow.waitFor();
    await modRow.click();
    await testPage.getByRole('button', { name: 'Resize panels', exact: true }).waitFor();
    for (const [width, height] of [[320, 740], [412, 915], [795, 339], [915, 412], [1280, 720], [412, 915]]) {
      await testPage.setViewportSize({ width, height });
      await testPage.waitForFunction(compact => Boolean(document.querySelector('[data-resize-handle]')) !== compact, width < 800);
      const searchWidth = await testPage.getByPlaceholder('Search mods', { exact: true }).evaluate(node => node.getBoundingClientRect().width);
      if (searchWidth < 176) throw new Error(`Mod search is unreadably narrow at ${width}px: ${searchWidth}px`);
      const state = await testPage.evaluate(() => {
        const workspace = document.querySelector('[data-launcher-workspace]');
        const toolbar = document.querySelector('[data-launcher-toolbar]');
        const row = document.querySelector('[data-launcher-mod-row]');
        const panels = row.closest('[data-launcher-workspace] > div.min-h-0');
        return { overflow: document.documentElement.scrollWidth > innerWidth || toolbar.scrollWidth > toolbar.clientWidth || panels.scrollWidth > panels.clientWidth, workspace: Boolean(workspace), rowVisible: row.getBoundingClientRect().width > 0 };
      });
      if (state.overflow || !state.workspace || !state.rowVisible) throw new Error(`Desktop responsive layout failed at ${width}: ${JSON.stringify(state)}`);
    }
    if (errors.length) throw new Error(errors.join('\n'));
    return 'Actual desktop LauncherPage passed: selected mod/config panel, resize removal/restore, toolbar and panel widths at 320/412/795/915/1280px';
  } catch (error) {
    throw new Error(`${error.message}\nPage errors: ${errors.join('; ')}\n${await testPage.locator('body').innerText()}`);
  } finally {
    await testPage.close();
  }
}
