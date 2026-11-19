// The launcher window. The work happens in the Rust side (commands get_state, get_notes, mark_notes_seen, play,
// pick_folder, reset_password); this file
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
  for (const id of ['name', 'password', 'forgot', 'invite', 'create', 'gamedir', 'browse', 'resolution', 'windowed']) $(id).disabled = busy;
}

$('create').addEventListener('change', () => {
  $('inviteRow').hidden = !$('create').checked;
  $('play').textContent = $('create').checked ? 'CREAR CUENTA Y JUGAR' : 'JUGAR';
});

// "¿Olvidaste tu contraseña?": a second form in place of the first, for the code an admin hands out.
function showReset(on) {
  $('form').hidden = on;
  $('resetForm').hidden = !on;
  if (on) {
    $('resetName').value = $('name').value.trim();
    for (const id of ['resetCode', 'resetPassword', 'resetPassword2']) $(id).value = '';
    ($('resetName').value ? $('resetCode') : $('resetName')).focus();
  }
  setStatus('');
}
$('forgot').addEventListener('click', () => showReset(true));
$('resetBack').addEventListener('click', () => showReset(false));

$('resetForm').addEventListener('submit', async (ev) => {
  ev.preventDefault();
  const name = $('resetName').value.trim(), code = $('resetCode').value.trim(), password = $('resetPassword').value;
  if (!name || !code || !password) return setStatus('Escribe tu nickname, el código y la contraseña nueva.');
  if (password.length < 6) return setStatus('La contraseña debe tener al menos 6 caracteres.');
  if (password !== $('resetPassword2').value) return setStatus('Las dos contraseñas no coinciden.');
  $('resetGo').disabled = true;
  setStatus('Cambiando la contraseña...');
  try {
    await invoke('reset_password', { name, code, password });
    showReset(false);
    $('name').value = name;
    $('password').value = '';
    $('password').focus();
    setStatus('Contraseña cambiada. Ya puedes entrar con la nueva.');
  } catch (e) {
    setStatus(String(e));
  } finally {
    $('resetGo').disabled = false;
  }
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

// Release notes from the server, newest first. Their text goes in as text, never as HTML. Entries newer than the
// last one the player saw get a mark; touching the list or pressing Play counts as seen (the marks stay until the
// window closes, so the player can still tell which ones were new).
const MONTHS = ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'];
const TAGS = { servidor: 'Servidor', launcher: 'Launcher', juego: 'Juego' };
let notesTop = 0, notesSeen = 0;

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

function showDate(d) {
  const [y, m, day] = d.split('-').map(Number);
  return MONTHS[m - 1] ? `${day} ${MONTHS[m - 1]} ${y}` : d;
}

async function loadNotes() {
  const r = await invoke('get_notes').catch(() => null);
  const notes = (r && r.notes) || [];
  $('notes').hidden = notes.length === 0;
  if (!notes.length) return;
  notesTop = notes[0].id;
  notesSeen = Math.max(notesSeen, r.seen || 0);
  const list = $('notesList');
  list.replaceChildren(...notes.map((n) => {
    const fresh = n.id > notesSeen;
    const a = el('article', fresh ? 'note new' : 'note');
    const meta = el('div', 'note-meta');
    if (n.tag) meta.append(el('span', 'note-tag', TAGS[n.tag]));
    if (n.date) meta.append(el('span', '', showDate(n.date)));
    if (fresh) meta.append(el('span', 'note-new', 'NUEVO'));
    a.append(meta, el('h2', '', n.title));
    if (n.items.length) {
      const ul = el('ul');
      ul.append(...n.items.map((t) => el('li', '', t)));
      a.append(ul);
    }
    return a;
  }));
  const fresh = notes.filter((n) => n.id > notesSeen).length;
  $('notesNew').hidden = fresh === 0;
  $('notesNew').textContent = fresh === 1 ? '1 nueva' : `${fresh} nuevas`;
}

function markNotesSeen() {
  if (notesTop <= notesSeen) return;
  notesSeen = notesTop;
  $('notesNew').hidden = true;
  invoke('mark_notes_seen', { id: notesTop });
}

// With Options and the invitation open there is no room left for the list: hide the panel rather than show half a header.
new ResizeObserver(() => $('notes').classList.toggle('squeezed', $('notesList').clientHeight < 24)).observe($('notes'));
for (const ev of ['pointerdown', 'wheel', 'keydown']) $('notesList').addEventListener(ev, markNotesSeen, { passive: true });
$('form').addEventListener('submit', markNotesSeen);

listen('status', (e) => setStatus(e.payload.text, e.payload.fraction));
listen('playing', () => { if (tauri) tauri.window.getCurrentWindow().minimize(); });
listen('done', (e) => {
  setBusy(false);
  setStatus(e.payload.error || '');
  $('password').value = '';
  if (!e.payload.error) { $('create').checked = false; $('create').dispatchEvent(new Event('change')); }
  if (tauri) { const w = tauri.window.getCurrentWindow(); w.unminimize(); w.setFocus(); }
  loadNotes(); // the server may have changed while the player was in the game
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
  loadNotes();
})();
