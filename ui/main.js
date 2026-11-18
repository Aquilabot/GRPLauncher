// The launcher window. The work happens in the Rust side (commands get_state, play, pick_folder); this file
// collects what the player typed and shows the progress events.
const $ = (id) => document.getElementById(id);
const tauri = window.__TAURI__;
const invoke = tauri ? tauri.core.invoke : async (cmd) => (cmd === 'get_state' ? { version: 0, name: '', gamedir: 'C:\\Users\\tu\\AppData\\Local\\GRP\\Ghost Recon Phantoms', resolution: 'auto', windowed: false, server: 'vista previa' } : null);
const listen = tauri ? tauri.event.listen : async () => {};

// The screen's own resolution, in real pixels (the page sees scaled ones on high-DPI screens).
const screenW = Math.round(window.screen.width * window.devicePixelRatio);
const screenH = Math.round(window.screen.height * window.devicePixelRatio);
const COMMON = [[1280, 720], [1366, 768], [1600, 900], [1920, 1080], [1920, 1200], [2560, 1080], [2560, 1440], [3440, 1440], [3840, 2160]];

function fillResolutions(saved) {
  const sel = $('resolution');
  sel.add(new Option(`Automática: la de tu pantalla (${screenW} × ${screenH})`, 'auto'));
  const list = COMMON.filter(([w, h]) => w <= screenW && h <= screenH);
  if (!list.some(([w, h]) => w === screenW && h === screenH)) list.push([screenW, screenH]);
  for (const [w, h] of list) sel.add(new Option(`${w} × ${h}`, `${w}x${h}`));
  if (saved && saved !== 'auto' && ![...sel.options].some((o) => o.value === saved)) sel.add(new Option(saved.replace('x', ' × '), saved));
  sel.value = saved || 'auto';
}

function setStatus(text, fraction) {
  $('status').textContent = text || '';
  const show = typeof fraction === 'number' && fraction >= 0;
  $('bar').hidden = !show;
  if (show) $('fill').style.width = `${Math.min(100, Math.max(0, fraction * 100)).toFixed(1)}%`;
}

function setBusy(busy) {
  $('play').disabled = busy;
  for (const id of ['name', 'password', 'invite', 'create', 'gamedir', 'browse', 'resolution', 'windowed']) $(id).disabled = busy;
}

$('create').addEventListener('change', () => {
  $('inviteRow').hidden = !$('create').checked;
  $('play').textContent = $('create').checked ? 'CREAR CUENTA Y JUGAR' : 'JUGAR';
});

$('browse').addEventListener('click', async () => {
  const dir = await invoke('pick_folder', { current: $('gamedir').value });
  if (dir) $('gamedir').value = dir;
});

$('form').addEventListener('submit', async (ev) => {
  ev.preventDefault();
  const name = $('name').value.trim(), password = $('password').value, creating = $('create').checked, invite = $('invite').value.trim();
  if (!name || !password) return setStatus('Escribe tu nickname y tu contraseña.');
  if (creating && password.length < 6) return setStatus('La contraseña debe tener al menos 6 caracteres.');
  if (creating && !invite) return setStatus('Escribe el código de invitación.');
  const res = $('resolution').value;
  const [w, h] = res === 'auto' ? [screenW, screenH] : res.split('x').map(Number);
  setBusy(true);
  setStatus('Conectando...');
  await invoke('play', { args: { name, password, invite: creating ? invite : null, gamedir: $('gamedir').value.trim(), resolution: res, width: w, height: h, windowed: $('windowed').checked } });
});

listen('status', (e) => setStatus(e.payload.text, e.payload.fraction));
listen('playing', () => { if (tauri) tauri.window.getCurrentWindow().minimize(); });
listen('done', (e) => {
  setBusy(false);
  setStatus(e.payload.error || '');
  $('password').value = '';
  if (!e.payload.error) { $('create').checked = false; $('create').dispatchEvent(new Event('change')); }
  if (tauri) { const w = tauri.window.getCurrentWindow(); w.unminimize(); w.setFocus(); }
});

(async () => {
  const s = await invoke('get_state');
  $('name').value = s.name;
  $('gamedir').value = s.gamedir;
  $('windowed').checked = s.windowed;
  $('server').textContent = `Servidor ${s.server}`;
  $('version').textContent = `launcher ${s.version}`;
  fillResolutions(s.resolution);
  (s.name ? $('password') : $('name')).focus();
})();
