const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const CATEGORY_ORDER = ['headphones', 'mouse', 'keyboard', 'gamepad', 'generic'];
const CURRENT_VERSION = '1.0.9';

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

function reflectControls(s) {
  document.querySelectorAll('.segmented[data-setting]').forEach((seg) => {
    const key = seg.dataset.setting;
    seg.querySelectorAll('button').forEach((btn) => {
      btn.classList.toggle('active', btn.dataset.value === s[key]);
    });
  });
  document.querySelectorAll('.swatches[data-setting]').forEach((sw) => {
    const key = sw.dataset.setting;
    sw.querySelectorAll('button').forEach((btn) => {
      btn.classList.toggle('active', btn.dataset.value === (s[key] || 'wyforce'));
    });
  });
  document.getElementById('toggle-startup').classList.toggle('on', !!s.launchAtLogin);
  document.getElementById('poll-select').value = String(s.pollIntervalSec || 30);

  const hiddenCount = (s.hiddenDevices || []).length;
  const hiddenRow = document.getElementById('hidden-row');
  if (hiddenRow) {
    hiddenRow.hidden = hiddenCount === 0;
    document.getElementById('hidden-count-hint').textContent =
      hiddenCount === 1 ? '1 dispositivo oculto.' : `${hiddenCount} dispositivos ocultos.`;
  }
}

async function updateSetting(patch) {
  settings = { ...settings, ...patch };
  applyAppearance(settings);
  reflectControls(settings);
  settings = await invoke('save_app_settings', { newSettings: settings });
}

function renderDevices() {
  const grid = document.getElementById('device-grid');
  const empty = document.getElementById('empty-state');
  const hiddenSet = new Set(settings.hiddenDevices || []);
  const visibleDevices = devices.filter((d) => !hiddenSet.has(d.id));

  if (!visibleDevices.length) {
    grid.innerHTML = '';
    empty.hidden = false;
    return;
  }

  empty.hidden = true;
  grid.replaceChildren(...visibleDevices.map((dev) => {
    const cat = (settings.customIcons && settings.customIcons[dev.id]) || dev.category || 'generic';
    const displayName = (settings.customNames && settings.customNames[dev.id]) || dev.name;
    const tone = dev.percent <= 15 ? 'bad' : dev.percent <= 30 ? 'warn' : 'ok';

    const card = document.createElement('div');
    card.className = 'device-card';
    card.dataset.tone = tone;

    const top = document.createElement('div');
    top.className = 'card-top';

    const iconBtn = document.createElement('div');
    iconBtn.className = 'dev-icon';
    iconBtn.title = 'Cambiar icono';
    iconBtn.innerHTML = window.DEVICE_ICONS[cat] || window.DEVICE_ICONS.generic;
    iconBtn.addEventListener('click', async () => {
      const next = CATEGORY_ORDER[(CATEGORY_ORDER.indexOf(cat) + 1) % CATEGORY_ORDER.length];
      settings = await invoke('set_device_icon', { id: dev.id, icon: next });
      renderDevices();
    });

    const meta = document.createElement('div');
    meta.className = 'dev-meta';

    const nameRow = document.createElement('div');
    nameRow.className = 'dev-name-row';

    const nameEl = document.createElement('div');
    nameEl.className = 'dev-name';
    nameEl.textContent = displayName;

    const actions = document.createElement('div');
    actions.className = 'card-actions';

    const editBtn = document.createElement('button');
    editBtn.className = 'mini-btn';
    editBtn.title = 'Cambiar nombre';
    editBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>';
    editBtn.addEventListener('click', () => {
      const input = document.createElement('input');
      input.className = 'dev-name-input';
      input.value = displayName;
      nameRow.replaceChildren(input);
      input.focus();
      input.select();

      const commit = async () => {
        settings = await invoke('set_device_name', { id: dev.id, name: input.value });
        renderDevices();
      };
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') commit();
        if (e.key === 'Escape') renderDevices();
      });
      input.addEventListener('blur', commit);
    });

    const hideBtn = document.createElement('button');
    hideBtn.className = 'mini-btn';
    hideBtn.title = 'Ocultar dispositivo';
    hideBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/><path d="M1 1l22 22"/></svg>';
    hideBtn.addEventListener('click', async () => {
      settings = await invoke('toggle_hide_device', { id: dev.id });
      reflectControls(settings);
      renderDevices();
    });

    actions.append(editBtn, hideBtn);
    nameRow.append(nameEl, actions);

    const connEl = document.createElement('div');
    connEl.className = 'dev-conn';
    connEl.textContent = dev.connection;
    meta.append(nameRow, connEl);

    const pct = document.createElement('div');
    pct.className = 'dev-pct';
    pct.textContent = `${dev.percent}%`;

    top.append(iconBtn, meta, pct);

    const track = document.createElement('div');
    track.className = 'bar-track';
    const fill = document.createElement('div');
    fill.className = 'bar-fill';
    fill.style.width = `${dev.percent}%`;
    track.append(fill);

    card.append(top, track);
    return card;
  }));
}

document.querySelectorAll('.nav-btn').forEach((btn) => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.nav-btn').forEach((b) => b.classList.remove('active'));
    document.querySelectorAll('.tab').forEach((t) => t.classList.remove('active'));
    btn.classList.add('active');
    document.getElementById(`tab-${btn.dataset.tab}`).classList.add('active');
  });
});

document.querySelectorAll('.segmented[data-setting] button').forEach((btn) => {
  btn.addEventListener('click', () => {
    updateSetting({ [btn.parentElement.dataset.setting]: btn.dataset.value });
  });
});

document.querySelectorAll('.swatches[data-setting] button').forEach((btn) => {
  btn.addEventListener('click', () => {
    updateSetting({ [btn.parentElement.dataset.setting]: btn.dataset.value });
  });
});

document.getElementById('toggle-startup').addEventListener('click', () => {
  updateSetting({ launchAtLogin: !settings.launchAtLogin });
});

document.getElementById('poll-select').addEventListener('change', (e) => {
  updateSetting({ pollIntervalSec: Number(e.target.value) });
});

document.getElementById('refresh-btn').addEventListener('click', async () => {
  devices = await invoke('refresh_devices');
  renderDevices();
});

document.getElementById('restore-hidden-btn').addEventListener('click', async () => {
  settings = await invoke('clear_hidden_devices');
  reflectControls(settings);
  renderDevices();
});

document.getElementById('quit-btn').addEventListener('click', () => invoke('quit_app'));

document.getElementById('check-updates-btn').addEventListener('click', async () => {
  const msg = document.getElementById('update-msg');
  msg.textContent = 'Comprobando...';
  try {
    const res = await fetch('https://api.github.com/repos/Tortusito/MyBattery-View/releases/latest');
    if (!res.ok) throw new Error();
    const data = await res.json();
    const latest = String(data.tag_name || '').replace(/^v/, '');
    const releaseUrl = data.html_url || 'https://github.com/Tortusito/MyBattery-View/releases/latest';
    if (latest && latest !== CURRENT_VERSION) {
      msg.innerHTML = '';
      const txt = document.createElement('div');
      txt.textContent = `Nueva versión disponible: v${latest}`;
      const btn = document.createElement('button');
      btn.className = 'update-link';
      btn.textContent = `Abrir Release v${latest}`;
      btn.addEventListener('click', () => invoke('open_external', { url: releaseUrl }));
      msg.append(txt, btn);
    } else {
      msg.textContent = `Estás en la última versión (v${CURRENT_VERSION})`;
    }
  } catch {
    msg.textContent = 'No se pudo comprobar ahora mismo.';
  }
});

(async () => {
  try {
    settings = await invoke('get_settings');
    applyAppearance(settings);
    reflectControls(settings);

    devices = await invoke('get_devices');
    renderDevices();

    await listen('devices-updated', (e) => {
      devices = e.payload || [];
      renderDevices();
    });
    await listen('settings-updated', (e) => {
      settings = e.payload || settings;
      applyAppearance(settings);
      reflectControls(settings);
      renderDevices();
    });
  } catch (err) {
    console.error('Init error:', err);
  }
})();
