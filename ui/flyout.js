const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let settings = {};
let devices = [];

function applyAppearance(s) {
  const root = document.documentElement;
  if (s.theme === 'dark' || s.theme === 'light') {
    root.dataset.theme = s.theme;
  } else {
    delete root.dataset.theme;
  }
  root.dataset.accent = s.accentColor || 'wyforce';
  document.body.classList.toggle('icon-tinted', s.iconStyle === 'tinted');
  document.body.classList.toggle('icon-bold', s.iconStyle === 'bold');
}

function render() {
  const rows = document.getElementById('rows');
  if (!devices.length) {
    rows.innerHTML = '<p class="empty">Sin dispositivos con batería detectados.</p>';
    return;
  }

  rows.replaceChildren(...devices.slice(0, 6).map((d) => {
    const cat = (settings.customIcons && settings.customIcons[d.id]) || d.category || 'generic';
    const tone = d.percent <= 15 ? 'bad' : d.percent <= 30 ? 'warn' : 'ok';

    const el = document.createElement('div');
    el.className = 'row';
    el.dataset.tone = tone;

    const icon = document.createElement('div');
    icon.className = 'icon';
    icon.innerHTML = window.DEVICE_ICONS[cat] || window.DEVICE_ICONS.generic;

    const body = document.createElement('div');
    const name = document.createElement('div');
    name.className = 'name';
    name.textContent = d.name;

    const track = document.createElement('div');
    track.className = 'track';
    const fill = document.createElement('span');
    fill.className = 'fill';
    fill.style.width = `${d.percent}%`;
    track.append(fill);
    body.append(name, track);

    const pct = document.createElement('div');
    pct.className = 'pct';
    pct.textContent = `${d.percent}%`;

    el.append(icon, body, pct);
    return el;
  }));
}

document.getElementById('refresh-btn').addEventListener('click', async () => {
  devices = await invoke('refresh_devices');
  render();
});

document.getElementById('open-btn').addEventListener('click', () => invoke('open_main_window'));
window.addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('hide_flyout'); });

(async () => {
  settings = await invoke('get_settings');
  applyAppearance(settings);
  devices = await invoke('get_devices');
  render();

  await listen('devices-updated', (e) => {
    devices = e.payload || [];
    render();
  });
  await listen('settings-updated', (e) => {
    settings = e.payload || settings;
    applyAppearance(settings);
    render();
  });
})();
