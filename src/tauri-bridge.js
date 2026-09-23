// Мост окна к Rust: window.kl поверх invoke()/listen() Tauri. app.js знает
// только window.kl — стенд вёрстки (.claude/mock-kl.js) подставляет свой.
(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const { open } = window.__TAURI__.dialog;
  window.kl = {
    overview: () => invoke('get_overview'),
    connect: () => invoke('connect'),
    disconnect: () => invoke('disconnect'),
    selectProfile: (id) => invoke('select_profile', { id }),
    selectServer: (profileId, name) => invoke('select_server', { profileId, name }),
    ping: (profileId) => invoke('ping', { profileId }),
    addLink: (input, name) => invoke('add_link', { input, name }),
    addFile: (path) => invoke('add_file', { path }),
    refreshSub: (id) => invoke('refresh_sub', { id }),
    removeProfile: (id) => invoke('remove_profile', { id }),
    restoreProfile: (profile, index, makeActive) => invoke('restore_profile', { profile, index, makeActive }),
    profileLink: (id) => invoke('profile_link', { id }),
    updateSettings: (patch) => invoke('update_settings', { patch }),
    setAutostart: (enabled) => invoke('set_autostart', { enabled }),
    retryKillSwitch: () => invoke('retry_kill_switch'),
    favicon: (host) => invoke('favicon', { host }),
    appInfo: () => invoke('app_info'),
    updateGeo: () => invoke('update_geo'),
    checkUpdate: () => invoke('check_update'),
    openUrl: (url) => invoke('open_url', { url }),
    licenses: () => invoke('licenses'),
    runningApps: () => invoke('running_apps'),
    appFromFile: (path) => invoke('app_from_file', { path }),
    getLogs: () => invoke('get_logs'),
    clearLogs: () => invoke('clear_logs'),
    restoreLogs: (list) => invoke('restore_logs', { list }),
    copyText: (text) => invoke('copy_text', { text }),
    openDataDir: () => invoke('open_data_dir'),
    minimize: () => invoke('window_minimize'),
    close: () => invoke('window_close'),
    pickConfig: async () =>
      (await open({ multiple: false, filters: [{ name: 'Конфиг mihomo / Clash', extensions: ['yaml', 'yml', 'json', 'txt'] }] })) || null,
    pickExe: async () => (await open({ multiple: false, filters: [{ name: 'Программа', extensions: ['exe'] }] })) || null,
    onFileDrop: (cb) => window.__TAURI__.webview.getCurrentWebview().onDragDropEvent((e) => cb(e.payload)),
    // События: core-status, traffic, logs, pings, profiles-changed, killswitch.
    on: (event, cb) => listen(event, (e) => cb(e.payload)),
  };
})();
