// Playwright MCP browser_run_code_unsafe(filename); Android Vite server on :1421.
// Mock only native IPC/events. The rendered React UI and user actions are real.
async (page) => {
  const equal = (actual, expected, message) => {
    if (actual !== expected) throw new Error(`${message}: expected ${expected}, got ${actual}`);
  };
  await page.setViewportSize({ width: 412, height: 915 });
  await page.addInitScript(() => {
    window.__runtimeFixture = {
      runtime_mode: 'internal', app_package: 'asta.hq_launcher',
      runtime_installed: false, runtime_version: null, installing: false,
      install_stage: '', install_progress: -1, install_error: null,
      steam_installed: false, game_installed: false, steam_ui: null,
      session_running: false, session_phase: 'stopped', session_error: null,
      microphone_enabled: false, microphone_permission: false,
      phantom_blocked: false, device: 'SM-S948N', android_version: '16',
    };
    window.__modsFixture = {
      game_version: 72, supported: true, installed: false, applying: false,
      mods: [{ dev: 'HQ', name: 'QuotaTools', enabled: true, installed: false }],
    };
    window.__nativeCalls = [];
    window.__nativeErrors = {};
    window.__statusFailure = '';
    window.__startupReady = new Promise(resolve => { window.__finishStartup = resolve; });
    window.__versionsFixture = [{ version: 81, installed: true, selected: true }, { version: 73, installed: false, selected: false }];
    const callbacks = new Map();
    const listeners = new Map();
    let callbackId = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event, id) => listeners.delete(id) };
    window.__emitModProgress = (payload) => {
      for (const [id, listener] of listeners) {
        if (listener.event === 'android-mod-progress') callbacks.get(listener.handler)?.({ id, event: listener.event, payload });
      }
    };
    window.__TAURI_INTERNALS__ = {
      transformCallback: (callback) => { callbacks.set(++callbackId, callback); return callbackId; },
      unregisterCallback: (id) => callbacks.delete(id),
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') { listeners.set(args.handler, args); return args.handler; }
        if (command === 'plugin:event|unlisten') return null;
        window.__nativeCalls.push({ command, args });
        if (command === 'get_android_runtime_status') {
          await window.__startupReady;
          if (window.__statusFailure) throw window.__statusFailure;
          return { ...window.__runtimeFixture };
        }
        if (window.__nativeErrors[command]) throw window.__nativeErrors[command];
        if (command === 'get_android_mod_state') return structuredClone(window.__modsFixture);
        if (command === 'get_android_versions') return { versions: structuredClone(window.__versionsFixture) };
        if (command === 'install_android_version') {
          window.__runtimeFixture.version_installing = true;
          window.__runtimeFixture.version_install = { version: args.version, installing: true, stage: 'Downloading game files' };
          window.__finishVersionInstall = () => {
            window.__runtimeFixture.version_installing = false;
            window.__runtimeFixture.version_install = { installing: false, stage: 'Installed' };
            window.__versionsFixture.forEach(item => { item.selected = item.version === args.version; if (item.selected) item.installed = true; });
            Object.assign(window.__modsFixture, { game_version: args.version, installed: false });
          };
          return null;
        }
        if (command === 'select_android_version') {
          window.__versionsFixture.forEach(item => { item.selected = item.version === args.version; });
          Object.assign(window.__modsFixture, { game_version: args.version, installed: true });
          return null;
        }
        if (command === 'set_android_microphone') {
          if (window.__runtimeFixture.session_running || window.__runtimeFixture.installing || window.__modsFixture.applying) throw new Error('Stop the runtime and finish mod operations before changing microphone settings.');
          window.__runtimeFixture.microphone_enabled = args.enabled;
          if (args.enabled) window.__runtimeFixture.microphone_permission = true;
          return null;
        }
        if (command === 'install_android_runtime') {
          Object.assign(window.__runtimeFixture, { installing: true, install_stage: 'Downloading Linux', install_progress: 34, install_error: null });
          return null;
        }
        if (['install_android_game', 'launch_android_game', 'open_android_developer_settings'].includes(command)) return null;
        if (command === 'stop_android_session') {
          Object.assign(window.__runtimeFixture, { session_running: false, session_phase: 'stopped' });
          return null;
        }
        if (command === 'set_android_mod_enabled') {
          if (!window.__modsFixture.installed) throw new Error('Install HQ mods first.');
          const mod = window.__modsFixture.mods.find((item) => item.dev === args.dev && item.name === args.name);
          if (!mod) throw new Error('Unknown mod');
          mod.enabled = args.enabled;
          return null;
        }
        if (command === 'install_android_mods') return new Promise((resolve) => {
          window.__finishModInstall = () => {
            window.__modsFixture.installed = true;
            window.__modsFixture.mods.forEach((mod) => { mod.installed = true; });
            resolve(null);
          };
        });
        throw new Error(`Unexpected Android command: ${command}`);
      },
    };
  });
  await page.goto('http://127.0.0.1:1421');
  await page.locator('[data-hq-loading="screen"] [data-hq-logo]').waitFor({ timeout: 3000 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  equal(await page.locator('[data-hq-loading="screen"] .hq-logo-fill').evaluate(node => getComputedStyle(node).animationName), 'none', 'Reduced motion keeps the logo static');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  equal(await page.locator('[data-hq-loading="screen"] .hq-logo-fill').evaluate(node => getComputedStyle(node).animationName), 'hq-logo-reveal', 'Normal motion reveals the HQ logo');
  await page.evaluate(() => window.__finishStartup());
  equal(await page.getByRole('heading', { name: /LETHAL/ }).count(), 0, 'Android uses the existing compact launcher workspace without a separate hero');
  await page.locator('[data-launcher-workspace]').waitFor();
  equal(await page.locator('[data-hq-loading="screen"]').count(), 0, 'Startup logo releases as soon as status is ready');
  const button = (name) => page.getByRole('button', { name, exact: true });
  const play = button('HQ Run');
  const refresh = button('Refresh');
  const microphone = page.getByRole('checkbox', { name: 'Enable microphone', exact: true });
  const countCalls = (command) => page.evaluate((name) => window.__nativeCalls.filter((call) => call.command === name).length, command);
  const changeStatus = async (patch) => {
    await page.evaluate((value) => {
      Object.assign(window.__runtimeFixture, value);
      window.dispatchEvent(new Event('focus'));
    }, patch);
  };
  const ready = (name) => page.waitForFunction((label) => [...document.querySelectorAll('button')].some((node) => node.textContent.trim() === label && !node.disabled), name);

  // Removing prerequisite gating would allow launch or a remote mod lookup here.
  await play.waitFor({ timeout: 10000 });
  equal(await page.locator('html').getAttribute('lang'), 'en', 'Android uses English like the desktop launcher');
  equal(await play.isDisabled(), true, 'Missing runtime prevents game launch');
  equal(await countCalls('get_android_mod_state'), 0, 'Base setup never fetches the mod catalog');
  await microphone.waitFor({ timeout: 3000 });
  equal(await microphone.isChecked(), false, 'Microphone is off by default');
  equal(await microphone.isEnabled(), true, 'A stopped runtime permits microphone setup before installation');
  await page.evaluate(() => { window.__nativeErrors.set_android_microphone = 'Microphone permission denied. Allow microphone access and try again.'; });
  await microphone.click();
  await page.getByRole('alert').filter({ hasText: 'Microphone permission denied' }).waitFor();
  equal(await microphone.isChecked(), false, 'Denied permission keeps the microphone off');
  equal(await microphone.isEnabled(), true, 'Denied permission permits retry');
  await page.evaluate(() => { delete window.__nativeErrors.set_android_microphone; });
  await microphone.focus();
  await microphone.press('Space');
  await page.waitForFunction(() => document.querySelector('input[aria-label="Enable microphone"]')?.checked);
  equal(await page.evaluate(() => window.__nativeCalls.some((call) => call.command === 'set_android_microphone' && call.args.enabled === true)), true, 'Explicit microphone opt-in reaches the native permission boundary');
  await microphone.uncheck();
  await page.waitForFunction(() => !document.querySelector('input[aria-label="Enable microphone"]')?.checked);
  equal(await page.evaluate(() => window.__nativeCalls.some((call) => call.command === 'set_android_microphone' && call.args.enabled === false)), true, 'Microphone opt-out is persisted');
  equal(await countCalls('get_android_mod_state'), 0, 'Microphone setup does not fetch mods before game installation');
  await button('Install Linux runtime').click();
  await page.getByRole('progressbar', { name: 'Runtime installation progress' }).waitFor();
  equal(await page.getByRole('progressbar', { name: 'Runtime installation progress' }).getAttribute('aria-valuenow'), '34', 'Native install progress is shown');
  await page.locator('[data-hq-logo][data-progress="34"]').waitFor();
  equal(await button('Installing…').isDisabled(), true, 'Install cannot be submitted twice');
  equal(await microphone.isDisabled(), true, 'Runtime installation locks microphone settings');
  equal(await countCalls('install_android_runtime'), 1, 'Only one install request');
  await changeStatus({ install_progress: -1, install_stage: 'Extracting Linux' });
  await page.waitForFunction(() => document.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow') === null);
  equal(await page.locator('[data-hq-loading="inline"] [data-hq-logo]').getAttribute('data-progress'), null, 'Unknown progress keeps an indeterminate logo instead of inventing a percentage');
  await changeStatus({ installing: false, runtime_installed: true, runtime_version: '0.5.0', steam_installed: true, install_stage: 'Complete', install_progress: 100 });
  await ready('Install Lethal Company');
  equal(await play.isDisabled(), true, 'Game installation remains required');
  await button('Install Lethal Company').click();
  equal(await countCalls('install_android_game'), 1, 'Game installation uses the fixed-game native command');
  equal(await countCalls('get_android_mod_state'), 0, 'Steam setup still does not fetch mods');
  await changeStatus({ session_running: true, session_phase: 'starting_steam', steam_ui: 'desktop' });
  await page.waitForFunction(() => document.querySelectorAll('button').length > 0 && [...document.querySelectorAll('button')].find(node => node.textContent.trim() === 'Install Lethal Company')?.disabled);
  equal(await button('Install Lethal Company').isDisabled(), true, 'Starting installer cannot be submitted twice');
  await changeStatus({ session_phase: 'ready' });
  await ready('Install Lethal Company');
  await button('Install Lethal Company').click();
  equal(await countCalls('install_android_game'), 2, 'An existing ready installer can be resumed');

  // Native state after returning from Steam is authoritative.
  await changeStatus({ game_installed: true, session_running: false, session_phase: 'idle' });
  await page.waitForFunction(() => ![...document.querySelectorAll('button')].some(node => node.textContent.trim() === 'Install Lethal Company'));
  equal(await countCalls('open_android_steam'), 0, 'The launcher does not open a generic Steam library');
  await ready('Install HQ mods');
  await page.getByText('Game version 72', { exact: false }).waitFor();
  const modToggle = page.getByRole('switch', { name: 'Enable QuotaTools' });
  equal(await modToggle.isDisabled(), true, 'Mod toggles require an installed HQ mod set');
  await button('Install HQ mods').click();
  await page.evaluate(() => window.__emitModProgress({ phase: 'download', detail: 'Downloading QuotaTools', progress: 52 }));
  await page.getByText('Downloading QuotaTools', { exact: false }).waitFor();
  equal(await page.getByRole('progressbar', { name: 'HQ mod installation progress' }).getAttribute('aria-valuenow'), '52', 'Mod progress is shown');
  equal(await modToggle.isDisabled(), true, 'Mod writes cannot overlap');
  equal(await microphone.isDisabled(), true, 'Mod application locks microphone settings');
  equal(await play.isDisabled(), true, 'Game launch waits for mod writes');
  await page.evaluate(() => window.__finishModInstall());
  await page.getByText('HQ mods installed', { exact: true }).waitFor();
  const search = page.getByRole('textbox', { name: 'Search mods', exact: true });
  await search.fill('missing-mod');
  equal(await page.locator('[data-launcher-mod-row]').count(), 0, 'Shared mod search filters Android rows');
  await search.fill('QUOTA');
  equal(await page.locator('[data-launcher-mod-row]').count(), 1, 'Shared search is case insensitive');
  // The same workspace reflows by width even while running the Android backend.
  for (const [width, height] of [[320, 740], [412, 915], [795, 339], [915, 412], [1280, 720]]) {
    await page.setViewportSize({ width, height });
    await page.waitForFunction((compact) => document.querySelector('[data-platform="android"]')?.dataset.layout === (compact ? 'compact' : 'wide'), width < 800);
    const layout = await page.evaluate(() => {
      const panels = document.querySelector('[data-launcher-panels]');
      const [mods, settings] = panels.children;
      const a = mods.getBoundingClientRect(), b = settings.getBoundingClientRect();
      return { overflow: panels.scrollWidth > panels.clientWidth, sideBySide: b.left >= a.right, stacked: b.top >= a.bottom, row: Boolean(document.querySelector('[data-launcher-mod-row]')), hero: Boolean(document.querySelector('h1#game-title')) };
    });
    equal(layout.overflow, false, `Panels fit ${width}px`);
    equal(width < 800 ? layout.stacked : layout.sideBySide, true, `Panels respond to ${width}px`);
    equal(layout.row, true, 'Existing shared mod row is used');
    equal(layout.hero, false, 'No separate mobile landing page');
    equal(await search.inputValue(), 'QUOTA', 'Width changes preserve the current search');
    equal(await search.evaluate(node => node.getBoundingClientRect().width >= 176), true, 'Search keeps a usable width');
  }
  await page.setViewportSize({ width: 412, height: 915 });
  await search.fill('');
  await ready('HQ Run');
  await modToggle.uncheck();
  await page.waitForFunction(() => document.querySelector('[aria-label="Enable QuotaTools"]')?.getAttribute('aria-checked') === 'false');
  equal(await page.evaluate(() => window.__nativeCalls.some((call) => call.command === 'set_android_mod_enabled' && call.args.dev === 'HQ' && call.args.name === 'QuotaTools' && call.args.enabled === false)), true, 'Mod toggle persists the selected mod');
  await ready('HQ Run');
  await play.click();
  equal(await countCalls('launch_android_game'), 1, 'Game uses the internal native command');
  await page.getByRole('status').filter({ hasText: 'Game launch requested' }).waitFor();
  equal(await button('Stop runtime').count(), 0, 'Accepted launch does not invent a running session');
  await changeStatus({ session_running: true, session_phase: 'running' });
  await button('Stop runtime').waitFor();
  equal(await modToggle.isDisabled(), true, 'Running session blocks mod writes');
  equal(await button('Reinstall HQ mods').isDisabled(), true, 'Running session blocks mod installation');
  equal(await play.isDisabled(), true, 'Running session blocks duplicate game launch');
  equal(await microphone.isDisabled(), true, 'Running session locks microphone settings');
  await button('Stop runtime').click();
  await ready('HQ Run');
  equal(await modToggle.isEnabled(), true, 'Stopped session allows mod changes');
  equal(await microphone.isEnabled(), true, 'Stopped session permits microphone changes for the next session');
  await changeStatus({ microphone_enabled: true, microphone_permission: false });
  await page.waitForFunction(() => !document.querySelector('input[aria-label="Enable microphone"]')?.disabled);
  equal(await microphone.isChecked(), false, 'Revoked permission cannot appear as an active microphone');
  await changeStatus({ microphone_permission: true });
  await page.waitForFunction(() => document.querySelector('input[aria-label="Enable microphone"]')?.checked);

  // Action failures keep their cause visible and permit retry.
  await page.evaluate(() => { window.__nativeErrors.launch_android_game = 'Unable to open the game executable'; });
  await play.click();
  await page.getByRole('alert').filter({ hasText: 'Unable to open the game executable' }).waitFor();
  equal(await play.isEnabled(), true, 'Failed launch permits retry');
  await page.evaluate(() => { delete window.__nativeErrors.launch_android_game; });
  await play.click();
  await page.getByRole('status').filter({ hasText: 'Game launch requested' }).waitFor();
  await page.evaluate(() => { Object.assign(window.__modsFixture, { game_version: null, supported: false, reason: 'Unable to determine the game version' }); });
  await refresh.click();
  await page.getByText('Unable to determine the game version', { exact: true }).waitFor();
  equal(await button('Reinstall HQ mods').isDisabled(), true, 'Unknown game version cannot install mods');
  equal(await modToggle.isDisabled(), true, 'Unsupported version cannot mutate mods');
  await changeStatus({ phantom_blocked: true, session_error: 'Android terminated the runtime process' });
  await button('Open Android Developer settings').waitFor();
  await button('Open Android Developer settings').click();
  equal(await countCalls('open_android_developer_settings'), 1, 'Blocked runtime links to native settings');
  equal(await play.isDisabled(), true, 'Blocked runtime cannot launch');
  await page.evaluate(() => { window.__statusFailure = 'Failed to check runtime status'; window.dispatchEvent(new Event('focus')); });
  await page.getByRole('alert').filter({ hasText: 'Failed to check runtime status' }).waitFor();
  equal(await play.isDisabled(), true, 'Failed status check removes stale launch access');
  equal(await microphone.isDisabled(), true, 'Unknown runtime status locks microphone settings');
  await page.evaluate(() => { window.__statusFailure = ''; });
  await changeStatus({ phantom_blocked: false, session_error: null, runtime_installed: false, steam_installed: false, game_installed: false, install_error: 'Not enough storage space' });
  await ready('Install Linux runtime');
  await page.getByRole('alert').filter({ hasText: 'Not enough storage space' }).waitFor();
  equal(await play.isDisabled(), true, 'Resume reflects removed game/runtime');
  equal(await modToggle.count(), 0, 'Resume clears stale mod state');
  await button('Install Linux runtime').click();
  await button('Installing…').waitFor();
  equal(await countCalls('install_android_runtime'), 2, 'Failed install permits retry');
  for (const [width, height] of [[320, 740], [412, 915], [915, 412], [1280, 720]]) {
    await page.setViewportSize({ width, height });
    equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, `No horizontal overflow at ${width}x${height}`);
    equal(await page.evaluate(() => [...document.querySelectorAll('button, label:has(input[type="checkbox"])')].every((node) => node.getBoundingClientRect().height >= 44)), true, `Touch controls are at least 44px at ${width}`);
  }
  equal(await page.getByText('DroidDeck', { exact: false }).count(), 0, 'No external app handoff remains');
  await page.evaluate(() => {
    Object.assign(window.__modsFixture, { game_version: 81, installed: true, supported: true, reason: null });
  });
  await changeStatus({ installing: false, runtime_installed: true, steam_installed: true, game_installed: true, install_error: null });
  const versionPicker = page.getByRole('combobox', { name: 'Game version', exact: true });
  await versionPicker.waitFor();
  await versionPicker.click();
  await page.getByRole('option', { name: 'v73 · Download', exact: true }).click();
  equal(await countCalls('install_android_version'), 1, 'Older version uses validated version installation command');
  await page.getByText('Downloading game files', { exact: false }).waitFor();
  equal(await play.isDisabled(), true, 'Version installation locks game launch');
  equal(await versionPicker.isDisabled(), true, 'Version installation locks selection');
  equal(await microphone.isDisabled(), true, 'Version installation locks microphone changes');
  await page.evaluate(() => window.__finishVersionInstall());
  await page.getByText('Game version 73', { exact: false }).waitFor();
  await versionPicker.click();
  await page.getByRole('option', { name: 'v81 · Installed', exact: true }).click();
  await page.getByText('Game version 81', { exact: false }).waitFor();
  equal(await countCalls('select_android_version'), 1, 'Installed version selection avoids a second download');
  await page.evaluate(() => {
    window.__modsFixture.mods.push({ dev: 'Zaggy1024', name: 'PathfindingLib', enabled: false, installed: true, blocked_reason: 'Disabled on Android: reproduced startup stall.' });
  });
  await refresh.click();
  await page.getByText('Disabled on Android: reproduced startup stall.', { exact: true }).waitFor();
  equal(await page.getByRole('switch', { name: 'Enable PathfindingLib', exact: true }).isDisabled(), true, 'A blocked startup-crashing plugin cannot be re-enabled');
  return 'Android UI passed: internal setup/progress, Steam, game/mod prerequisites, mod apply/toggle, microphone opt-in/denial/retry/locks, errors/retry, resume, responsive touch controls';
}
