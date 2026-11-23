// The launcher window. The work happens in the Rust side (commands get_state, get_notes, mark_notes_seen, play,
// pick_folder, reset_password, set_lang, open_link); this file collects what the player typed, shows the progress
// events and draws the two diagrams (the terrain of the home view, the view cone of the settings).
const $ = (id) => document.getElementById(id);
const tauri = window.__TAURI__;
const PREVIEW = { version: 0, name: '', gamedir: 'C:\\Users\\tu\\AppData\\Local\\GRP\\Ghost Recon Phantoms', resolution: 'auto', windowed: false, mousesens: '', clientmods: true, maxfps: '', fov: '', focallength: false, adsmult: '', scopemult: '', dxvk: true, server: '56.127.0.17', lang: '' };
// Outside the launcher (the page opened in a browser, to look at it): the state above and three notes of the real log.
const PREVIEW_NOTES = { seen: 25, notes: [
  { id: 26, date: '2026-11-23', tag: 'servidor', title: 'Bots más naturales', items: ['Los bots se mueven más como un jugador: corren y esprintan a velocidad de jugador, trepan y caen en vez de cambiar de altura de golpe.', 'Ya no se meten dentro de los muros ni se quedan quietos cuando les disparan.'] },
  { id: 25, date: '2026-11-23', tag: 'juego', title: 'Cajas sorpresa y control del ratón', items: ['Ya funcionan las cajas sorpresa (Mystery Boxes) en la tienda.', 'Campo de visión configurable de 70° a 110° y multiplicadores propios para apuntar.'] },
  { id: 24, date: '2026-11-22', tag: 'launcher', title: 'Registro abierto', items: ['Crear una cuenta nueva ya no requiere código de invitación.'] },
] };
const invoke = tauri ? tauri.core.invoke : async (cmd) => (cmd === 'get_state' ? PREVIEW : cmd === 'get_notes' ? PREVIEW_NOTES : null);
const listen = tauri ? tauri.event.listen : async () => {};

// ---- the two languages. Spanish is the server's home language; English for everybody else.
const TEXT = {
  es: {
    'nav.label': 'Secciones', 'nav.home': 'Inicio', 'nav.settings': 'Ajustes', 'lang.label': 'Idioma',
    'server.name': 'Servidor de la comunidad', 'server.place': 'São Paulo · 60 Hz',
    'notes.title': 'Novedades', 'notes.empty': 'Todavía no hay novedades publicadas.', 'notes.new1': '1 nueva', 'notes.newN': '{n} nuevas', 'notes.fresh': 'Nuevo',
    'tag.servidor': 'Servidor', 'tag.launcher': 'Launcher', 'tag.juego': 'Juego',
    'set.game': 'Juego', 'set.gamedir': 'Carpeta del juego', 'set.browse': 'Buscar', 'set.gamedir.hint': 'Si el juego no está ahí, se descarga (unos 4 GB).',
    'set.resolution': 'Resolución', 'set.windowed': 'En ventana', 'set.res.auto': 'Automática: la de tu pantalla ({w} × {h})',
    'set.dxvk': 'Vulkan (DXVK)', 'set.on': 'Activado', 'set.dxvk.hint': 'Si tu tarjeta no tiene Vulkan 1.3 o el juego no abre así, se usa DirectX 9.', 'set.dxvk.linux': 'En Linux el juego ya usa Vulkan con el DXVK de Proton.',
    'set.mouse': 'Ratón', 'set.sens': 'Sensibilidad exacta', 'set.sens.placeholder': 'Sin cambios (la del juego)', 'set.calc': 'Calcular', 'set.calc.close': 'Cerrar',
    'set.sens.hint': 'El valor que el juego usa por dentro, sin los pasos de 0,05 del menú. Vacío conserva el del juego.',
    'calc.from': 'Convertir desde', 'calc.cm360': 'cm por vuelta (360°)', 'calc.dpi': 'DPI del ratón', 'calc.apply': 'Aplicar', 'calc.in': 'Sensibilidad en {game}',
    'calc.empty': 'Escribe un valor para calcular.', 'calc.now': 'Ahora: {cm} cm por vuelta a {dpi} DPI', 'calc.result': 'En GRP: {v}',
    'set.mods': 'Mejoras del cliente', 'set.mods.on': 'Activadas', 'set.mods.hint': 'Resoluciones de más de 1920 × 1200, más de 60 FPS, ratón sin tope de giro y los ajustes de abajo. Desactivadas, el juego queda como el original.',
    'set.fov': 'Campo de visión', 'set.fov.hint': 'Horizontal, al caminar o correr (en 16:9). Las miras conservan su zoom.', 'set.fov.stock': 'Original 79°',
    'cone.title': 'Campo de visión de {deg}° comparado con el original de 79°',
    'set.maxfps': 'Límite de FPS', 'fps.auto': 'Automático (frecuencia del monitor)', 'fps.none': 'Sin límite',
    'set.ads': 'Sensibilidad al apuntar', 'set.focal': 'Escalar por distancia focal', 'set.focal.hint': 'Mantiene la misma distancia de ratón por distancia en pantalla al apuntar, en lugar de la curva original del juego.', 'set.adsmult': 'Multiplicador con mira', 'set.scopemult': 'Multiplicador con telescópica',
    'acc.label': 'Cuenta', 'acc.login': 'Entrar', 'acc.create': 'Cuenta nueva', 'acc.name': 'Nickname', 'acc.password': 'Contraseña',
    'acc.create.hint': 'Gratis y sin correo. La contraseña, de 6 caracteres o más.', 'acc.forgot': '¿Olvidaste tu contraseña?', 'acc.invite': 'Código de invitación',
    'play': 'Jugar', 'play.create': 'Crear cuenta y jugar',
    'reset.title': 'Cambiar la contraseña', 'reset.intro': 'Pide un código a un administrador en Discord. Sirve una sola vez y caduca en 24 horas.',
    'reset.code': 'Código', 'reset.new': 'Contraseña nueva', 'reset.again': 'Repite la contraseña nueva', 'reset.go': 'Cambiar contraseña', 'reset.back': 'Volver',
    'links.label': 'Enlaces', 'links.status': 'Estado del servidor', 'links.source': 'Código fuente', 'links.support': 'Apoyar el servidor',
    'legal': 'Proyecto de la comunidad, sin relación con Ubisoft.', 'version': 'Launcher {v}',
    'msg.need': 'Escribe tu nickname y tu contraseña.', 'msg.short': 'La contraseña debe tener al menos 6 caracteres.', 'msg.invite': 'Escribe el código de invitación.',
    'msg.connecting': 'Conectando...', 'msg.reset.need': 'Escribe tu nickname, el código y la contraseña nueva.', 'msg.reset.differ': 'Las dos contraseñas no coinciden.',
    'msg.reset.busy': 'Cambiando la contraseña...', 'msg.reset.done': 'Contraseña cambiada. Ya puedes entrar con la nueva.',
    months: ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'],
  },
  en: {
    'nav.label': 'Sections', 'nav.home': 'Home', 'nav.settings': 'Settings', 'lang.label': 'Language',
    'server.name': 'Community server', 'server.place': 'São Paulo · 60 Hz',
    'notes.title': 'What changed', 'notes.empty': 'No release notes published yet.', 'notes.new1': '1 new', 'notes.newN': '{n} new', 'notes.fresh': 'New',
    'tag.servidor': 'Server', 'tag.launcher': 'Launcher', 'tag.juego': 'Game',
    'set.game': 'Game', 'set.gamedir': 'Game folder', 'set.browse': 'Browse', 'set.gamedir.hint': 'If the game is not there, it is downloaded (about 4 GB).',
    'set.resolution': 'Resolution', 'set.windowed': 'Windowed', 'set.res.auto': 'Automatic: your screen ({w} × {h})',
    'set.dxvk': 'Vulkan (DXVK)', 'set.on': 'On', 'set.dxvk.hint': 'If your graphics card has no Vulkan 1.3, or the game does not open with it, DirectX 9 is used.', 'set.dxvk.linux': 'On Linux the game already runs on Vulkan through Proton’s DXVK.',
    'set.mouse': 'Mouse', 'set.sens': 'Exact sensitivity', 'set.sens.placeholder': 'Unchanged (the game’s own)', 'set.calc': 'Convert', 'set.calc.close': 'Close',
    'set.sens.hint': 'The value the game uses inside, without the 0.05 steps of its menu. Empty keeps the game’s own.',
    'calc.from': 'Convert from', 'calc.cm360': 'cm per turn (360°)', 'calc.dpi': 'Mouse DPI', 'calc.apply': 'Apply', 'calc.in': 'Sensitivity in {game}',
    'calc.empty': 'Type a value to convert.', 'calc.now': 'Now: {cm} cm per turn at {dpi} DPI', 'calc.result': 'In GRP: {v}',
    'set.mods': 'Client improvements', 'set.mods.on': 'On', 'set.mods.hint': 'Resolutions above 1920 × 1200, more than 60 FPS, no turn-speed cap on the mouse, and the settings below. Off, the game is the original.',
    'set.fov': 'Field of view', 'set.fov.hint': 'Horizontal, walking or running (at 16:9). Sights keep their zoom.', 'set.fov.stock': 'Original 79°',
    'cone.title': 'A {deg}° field of view against the original 79°',
    'set.maxfps': 'FPS limit', 'fps.auto': 'Automatic (the monitor’s refresh rate)', 'fps.none': 'No limit',
    'set.ads': 'Sensitivity when aiming', 'set.focal': 'Scale by focal length', 'set.focal.hint': 'Keeps the same mouse travel per distance on screen when aiming, instead of the game’s original curve.', 'set.adsmult': 'Multiplier with a sight', 'set.scopemult': 'Multiplier with a scope',
    'acc.label': 'Account', 'acc.login': 'Sign in', 'acc.create': 'New account', 'acc.name': 'Nickname', 'acc.password': 'Password',
    'acc.create.hint': 'Free, no e-mail needed. The password, 6 characters or more.', 'acc.forgot': 'Forgot your password?', 'acc.invite': 'Invitation code',
    'play': 'Play', 'play.create': 'Create account and play',
    'reset.title': 'Change the password', 'reset.intro': 'Ask an administrator on Discord for a code. It works once and expires in 24 hours.',
    'reset.code': 'Code', 'reset.new': 'New password', 'reset.again': 'Repeat the new password', 'reset.go': 'Change password', 'reset.back': 'Back',
    'links.label': 'Links', 'links.status': 'Server status', 'links.source': 'Source code', 'links.support': 'Support the server',
    'legal': 'A community project, not affiliated with Ubisoft.', 'version': 'Launcher {v}',
    'msg.need': 'Type your nickname and your password.', 'msg.short': 'The password needs at least 6 characters.', 'msg.invite': 'Type the invitation code.',
    'msg.connecting': 'Connecting...', 'msg.reset.need': 'Type your nickname, the code and the new password.', 'msg.reset.differ': 'The two passwords are not the same.',
    'msg.reset.busy': 'Changing the password...', 'msg.reset.done': 'Password changed. You can sign in with the new one.',
    months: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
  },
};
let lang = 'es';
const t = (key, vars) => {
  let s = TEXT[lang][key] ?? TEXT.es[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, v);
  return s;
};
const fresh = []; // what to write again when the language changes
function applyLang(next) {
  lang = TEXT[next] ? next : 'es';
  document.documentElement.lang = lang;
  for (const e of document.querySelectorAll('[data-i18n]')) e.textContent = t(e.dataset.i18n);
  for (const e of document.querySelectorAll('[data-i18n-aria]')) e.setAttribute('aria-label', t(e.dataset.i18nAria));
  for (const e of document.querySelectorAll('[data-i18n-placeholder]')) e.placeholder = t(e.dataset.i18nPlaceholder);
  for (const b of document.querySelectorAll('.lang button')) b.setAttribute('aria-pressed', String(b.dataset.lang === lang));
  for (const f of fresh) f();
}
for (const b of document.querySelectorAll('.lang button')) b.addEventListener('click', () => {
  if (b.dataset.lang === lang) return;
  applyLang(b.dataset.lang);
  invoke('set_lang', { lang }).catch(() => {});
});

// ---- the two views of the left field
function showView(view) {
  $('app').dataset.view = view;
  $('viewHome').hidden = view !== 'home';
  $('viewSettings').hidden = view !== 'settings';
  $('tabHome').setAttribute('aria-selected', String(view === 'home'));
  $('tabSettings').setAttribute('aria-selected', String(view === 'settings'));
}
$('tabHome').addEventListener('click', () => showView('home'));
$('tabSettings').addEventListener('click', () => showView('settings'));
for (const tab of [$('tabHome'), $('tabSettings')]) tab.addEventListener('keydown', (ev) => {
  if (ev.key !== 'ArrowLeft' && ev.key !== 'ArrowRight') return;
  const other = tab === $('tabHome') ? $('tabSettings') : $('tabHome');
  other.focus(); other.click();
});

// ---- the terrain of the home view: rows of a height field, far to near, each hiding what lies behind it
const SERVER_AT = { u: 0.4, v: 0.42 };
function height(u, v) {
  const peak = Math.exp(-(((u - SERVER_AT.u) ** 2) / 0.016 + ((v - SERVER_AT.v) ** 2) / 0.07));
  const ridge = Math.exp(-(((u - 0.86) ** 2) / 0.03 + ((v - 0.2) ** 2) / 0.05));
  return 0.42 * Math.sin(u * 5.1 + 1.3) * Math.cos(v * 3.2 + 0.4) + 0.2 * Math.sin(u * 11.7 + v * 6.1) + 0.1 * Math.sin(u * 23 - v * 9) + 1.5 * peak + 0.9 * ridge;
}
function project(u, v) {
  const scale = 0.4 + 0.6 * v;
  return [330 + (u - 0.5) * 660 * 1.6 * scale, 92 + Math.pow(v, 1.3) * 128 - height(u, v) * 40 * scale];
}
function drawTerrain() {
  const NS = 'http://www.w3.org/2000/svg', svg = $('terrain'), ROWS = 22, COLS = 72;
  const parts = [];
  for (let r = 0; r <= ROWS; r++) {
    const v = r / ROWS;
    let d = '';
    for (let c = 0; c <= COLS; c++) {
      const [x, y] = project(c / COLS, v);
      d += `${c ? 'L' : 'M'}${x.toFixed(1)} ${y.toFixed(1)}`;
    }
    const fill = document.createElementNS(NS, 'path');
    fill.setAttribute('class', 'ground');
    fill.setAttribute('d', `${d}L1200 260L-600 260Z`);
    const line = document.createElementNS(NS, 'path');
    line.setAttribute('class', Math.abs(v - SERVER_AT.v) < 0.5 / ROWS ? 'line lit' : 'line');
    line.setAttribute('d', d);
    line.setAttribute('pathLength', '1');
    line.style.setProperty('--i', r);
    line.style.opacity = (0.28 + 0.72 * v).toFixed(2);
    parts.push(fill, line);
  }
  svg.replaceChildren(...parts);
  const [x, y] = project(SERVER_AT.u, Math.round(SERVER_AT.v * ROWS) / ROWS);
  $('serverTag').style.left = `${(x - 5).toFixed(1)}px`;
  $('serverTag').style.top = `${(y - 41).toFixed(1)}px`;
}

// The screen's own resolution, in real pixels (the page sees scaled ones on high-DPI screens).
const screenW = Math.round(window.screen.width * window.devicePixelRatio);
const screenH = Math.round(window.screen.height * window.devicePixelRatio);
const COMMON = [[1280, 720], [1366, 768], [1600, 900], [1920, 1080], [1920, 1200], [2560, 1080], [2560, 1440], [3440, 1440], [3840, 2160]];

function fillResolutions(saved) {
  const sel = $('resolution');
  const auto = new Option('', 'auto');
  sel.add(auto);
  fresh.push(() => { auto.textContent = t('set.res.auto', { w: screenW, h: screenH }); });
  const list = COMMON.filter(([w, h]) => w <= screenW && h <= screenH);
  if (!list.some(([w, h]) => w === screenW && h === screenH)) list.push([screenW, screenH]);
  for (const [w, h] of list) sel.add(new Option(`${w} × ${h}`, `${w}x${h}`));
  if (saved && saved !== 'auto' && ![...sel.options].some((o) => o.value === saved)) sel.add(new Option(saved.replace('x', ' × '), saved));
  sel.value = saved || 'auto';
}

// Exact MouseSensitivity converter: GRP turns MouseSensitivity / 200 radians per mouse count (no acceleration).
const GRP_DEG_PER_SENS = 180 / (200 * Math.PI);
const GAME_YAW = {
  thefinals: { game: 'The Finals', placeholder: '37', yaw: 0.001 },
  cs2: { game: 'CS2 / Apex', placeholder: '2.0', yaw: 0.022 },
  valorant: { game: 'Valorant', placeholder: '0.35', yaw: 0.07 },
  ow2: { game: 'Overwatch 2 / CoD', placeholder: '5.5', yaw: 0.0066 },
  fortnite: { game: 'Fortnite (%)', placeholder: '6.5', yaw: 0.005555 },
  r6: { game: 'Rainbow Six Siege', placeholder: '10', yaw: 0.005729578 },
};
let sensComputed = null;
const parseNum = (s) => {
  const n = Number(String(s || '').trim().replace(',', '.'));
  return Number.isFinite(n) && n > 0 ? n : NaN;
};
function updateSensCalc() {
  const mode = $('sensFrom').value;
  const isCm = mode === 'cm360';
  $('sensDpiCol').hidden = !isCm;
  if (isCm) {
    $('sensInputLabel').textContent = 'cm / 360°';
    $('sensInput').placeholder = '30';
  } else {
    $('sensInputLabel').textContent = t('calc.in', { game: GAME_YAW[mode].game });
    $('sensInput').placeholder = GAME_YAW[mode].placeholder;
  }
  const val = parseNum($('sensInput').value);
  const dpi = parseNum($('sensDpi').value);
  let sens = NaN;
  if (isCm && val > 0 && dpi > 0) sens = (400 * Math.PI * 2.54) / (val * dpi);
  else if (!isCm && val > 0) sens = (val * GAME_YAW[mode].yaw) / GRP_DEG_PER_SENS;
  const out = $('sensPreview');
  if (Number.isFinite(sens) && sens > 0) {
    sensComputed = Math.min(10, Math.max(0.001, sens));
    out.textContent = t('calc.result', { v: sensComputed.toFixed(6) });
    out.className = 'ready';
    $('sensApply').disabled = $('play').disabled;
  } else {
    sensComputed = null;
    out.className = '';
    $('sensApply').disabled = true;
    const cur = parseNum($('mousesens').value);
    if (isCm && cur > 0 && dpi > 0) {
      const curCm = (400 * Math.PI * 2.54) / (cur * dpi);
      out.textContent = t('calc.now', { cm: curCm.toFixed(1), dpi: Math.round(dpi) });
    } else {
      out.textContent = t('calc.empty');
    }
  }
}
function showSensCalc(show) {
  $('sensCalc').hidden = !show;
  $('sensToggle').setAttribute('aria-expanded', String(show));
  $('sensToggle').textContent = t(show ? 'set.calc.close' : 'set.calc');
}
function applySensCalc() {
  if (sensComputed === null) return;
  $('mousesens').value = sensComputed.toFixed(6);
  showSensCalc(false);
  $('mousesens').focus();
}
$('sensToggle').addEventListener('click', () => {
  const show = $('sensCalc').hidden;
  showSensCalc(show);
  if (show) { updateSensCalc(); $('sensInput').focus(); }
});
$('sensFrom').addEventListener('change', () => { $('sensInput').value = ''; updateSensCalc(); $('sensInput').focus(); });
for (const id of ['sensInput', 'sensDpi', 'mousesens']) $(id).addEventListener('input', updateSensCalc);
for (const id of ['sensInput', 'sensDpi']) $(id).addEventListener('keydown', (ev) => {
  if (ev.key === 'Enter') { ev.preventDefault(); applySensCalc(); }
});
$('sensApply').addEventListener('click', applySensCalc);
fresh.push(() => { showSensCalc(!$('sensCalc').hidden); updateSensCalc(); });

// "Mejoras del cliente": our version.dll and its frame cap, FOV, and ADS sensitivity settings.
function fillMaxFps(saved) {
  const sel = $('maxfps');
  const auto = new Option('', '');
  sel.add(auto);
  for (const n of [60, 120, 144, 165, 240]) sel.add(new Option(`${n} FPS`, String(n)));
  let none = null;
  if (saved && ![...sel.options].some((o) => o.value === saved)) {
    const o = new Option(`${saved} FPS`, saved);
    if (saved === '0') none = o;
    sel.add(o);
  }
  fresh.push(() => { auto.textContent = t('fps.auto'); if (none) none.textContent = t('fps.none'); });
  sel.value = saved || '';
}

// Field of view: step 0 of the slider is the game's own (about 79 degrees), then 70 to 110 in fives. A value saved by
// hand that is none of those stays as it is until the player moves the slider.
const STOCK_FOV = 79.3;
const FOV_STEPS = ['', '70', '75', '80', '85', '90', '95', '100', '105', '110'];
let fovOdd = '';
const fovValue = () => fovOdd || FOV_STEPS[Number($('fov').value)];
function setFov(saved) {
  const at = FOV_STEPS.indexOf(saved || '');
  fovOdd = at < 0 ? saved : '';
  $('fov').value = at < 0 ? FOV_STEPS.reduce((best, s, i) => (s && Math.abs(s - saved) < Math.abs(FOV_STEPS[best] - saved) ? i : best), 1) : at;
  drawCone();
}
function wedge(deg, r) {
  const a = (deg / 2) * Math.PI / 180, x = r * Math.sin(a), y = 138 - r * Math.cos(a);
  return { arc: `M${(140 - x).toFixed(1)} ${y.toFixed(1)}A${r} ${r} 0 0 1 ${(140 + x).toFixed(1)} ${y.toFixed(1)}`, x, y };
}
function drawCone() {
  const cone = $('cone'), off = $('fov').disabled, v = fovValue(), deg = off || !v ? STOCK_FOV : Number(v);
  const w = wedge(deg, 124), s = wedge(STOCK_FOV, 124);
  cone.classList.toggle('off', off);
  cone.querySelector('.wedge').setAttribute('d', `M140 138L${(140 - w.x).toFixed(1)} ${w.y.toFixed(1)}A124 124 0 0 1 ${(140 + w.x).toFixed(1)} ${w.y.toFixed(1)}Z`);
  cone.querySelector('.edge').setAttribute('d', `M${(140 - w.x).toFixed(1)} ${w.y.toFixed(1)}L140 138L${(140 + w.x).toFixed(1)} ${w.y.toFixed(1)}`);
  cone.querySelector('.stock').setAttribute('d', `M${(140 - s.x).toFixed(1)} ${s.y.toFixed(1)}L140 138L${(140 + s.x).toFixed(1)} ${s.y.toFixed(1)}`);
  cone.querySelector('.rings').innerHTML = [42, 83, 124].map((r) => `<path d="${wedge(120, r).arc}"/>`).join('');
  // a tick every ten degrees on the outer ring, a longer one every thirty
  let ticks = '';
  for (let a = -60; a <= 60; a += 10) {
    const rad = a * Math.PI / 180, r2 = a % 30 ? 129 : 133;
    ticks += `<path d="M${(140 + 124 * Math.sin(rad)).toFixed(1)} ${(138 - 124 * Math.cos(rad)).toFixed(1)}L${(140 + r2 * Math.sin(rad)).toFixed(1)} ${(138 - r2 * Math.cos(rad)).toFixed(1)}"/>`;
  }
  cone.querySelector('.ticks').innerHTML = ticks;
  // the figure, tied to the right edge of the cone by a leader; the game's own, named on the left
  const lx = 140 + w.x, ly = w.y, right = Math.min(274, lx + 26);
  cone.querySelector('.lead').setAttribute('d', `M${lx.toFixed(1)} ${ly.toFixed(1)}L${(lx + 9).toFixed(1)} ${(ly - 9).toFixed(1)}H${right.toFixed(1)}`);
  const now = $('coneNow');
  now.setAttribute('x', right.toFixed(1)); now.setAttribute('y', (ly - 14).toFixed(1)); now.setAttribute('text-anchor', 'end');
  now.textContent = `${Math.round(deg)}°`;
  const stock = $('coneStock');
  stock.setAttribute('x', (140 - s.x - 6).toFixed(1)); stock.setAttribute('y', (s.y - 5).toFixed(1)); stock.setAttribute('text-anchor', 'end');
  stock.textContent = off || !v || Math.round(deg) === 79 ? '' : '79°';
  $('coneTitle').textContent = t('cone.title', { deg: Math.round(deg) });
  $('fovOut').textContent = !off && v ? `${v}°` : t('set.fov.stock');
}
$('fov').addEventListener('input', () => { fovOdd = ''; drawCone(); });
fresh.push(drawCone);

const CLIENTMOD_CONTROLS = ['maxfps', 'fov', 'focallength', 'adsmult', 'scopemult'];
function syncClientMods(busy = false) {
  const off = busy || !$('clientmods').checked;
  for (const id of CLIENTMOD_CONTROLS) $(id).disabled = off;
  drawCone();
}
$('clientmods').addEventListener('change', () => syncClientMods(false));

// "Vulkan (DXVK)": on Windows a choice; on Linux the game always runs on Proton's DXVK, so it shows as on and fixed.
const LINUX = /Linux/.test(navigator.userAgent);
function showDxvk(on) {
  $('dxvk').checked = LINUX || on;
}
fresh.push(() => { if (LINUX) $('dxvkNote').textContent = t('set.dxvk.linux'); });

// What the status line says: plain progress, a fault (red), or good news (the password changed).
function setStatus(text, fraction, kind) {
  $('status').textContent = text || '';
  $('status').className = kind || '';
  // The gauge: a measured part when there is one, a sweep while the launcher works on something it cannot measure.
  const show = typeof fraction === 'number' && fraction >= 0, busy = 'busy' in $('app').dataset;
  $('gauge').hidden = !show && !busy;
  $('bar').classList.toggle('sweep', busy && !show);
  $('fill').style.transform = show ? `scaleX(${Math.min(1, Math.max(0, fraction)).toFixed(3)})` : '';
  $('pct').textContent = show ? `${Math.round(Math.min(1, Math.max(0, fraction)) * 100)} %` : '';
}

function setBusy(busy) {
  if (busy) $('app').dataset.busy = ''; else delete $('app').dataset.busy;
  $('play').disabled = busy;
  for (const id of ['name', 'password', 'forgot', 'invite', 'create', 'modeLogin', 'gamedir', 'browse', 'resolution', 'windowed', 'mousesens', 'sensToggle', 'sensFrom', 'sensInput', 'sensDpi', 'clientmods']) $(id).disabled = busy;
  $('sensApply').disabled = busy || sensComputed === null;
  syncClientMods(busy);
  $('dxvk').disabled = busy || LINUX;
}

function syncMode() {
  const creating = $('create').checked;
  $('playLabel').textContent = t(creating ? 'play.create' : 'play');
  $('play').classList.toggle('long', creating);
  $('createHint').hidden = !creating;
  $('forgot').hidden = creating;
}
for (const id of ['create', 'modeLogin']) $(id).addEventListener('change', () => {
  $('inviteRow').hidden = true;
  $('invite').value = '';
  syncMode();
});
fresh.push(syncMode);

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
  if (!name || !code || !password) return setStatus(t('msg.reset.need'), -1, 'fault');
  if (password.length < 6) return setStatus(t('msg.short'), -1, 'fault');
  if (password !== $('resetPassword2').value) return setStatus(t('msg.reset.differ'), -1, 'fault');
  $('resetGo').disabled = true;
  setStatus(t('msg.reset.busy'));
  try {
    await invoke('reset_password', { name, code, password });
    showReset(false);
    $('name').value = name;
    $('password').value = '';
    $('password').focus();
    setStatus(t('msg.reset.done'), -1, 'good');
  } catch (e) {
    setStatus(String(e), -1, 'fault');
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
  if (!name || !password) return setStatus(t('msg.need'), -1, 'fault');
  if (creating && password.length < 6) return setStatus(t('msg.short'), -1, 'fault');
  if (creating && !$('inviteRow').hidden && !invite) return setStatus(t('msg.invite'), -1, 'fault');
  const res = $('resolution').value;
  const [w, h] = res === 'auto' ? [screenW, screenH] : res.split('x').map(Number);
  setBusy(true);
  setStatus(t('msg.connecting'));
  await invoke('play', { args: { name, password, create: creating, invite: creating && invite ? invite : null, gamedir: $('gamedir').value.trim(), resolution: res, width: w, height: h, windowed: $('windowed').checked, mousesens: $('mousesens').value.trim(), clientmods: $('clientmods').checked, maxfps: $('maxfps').value, fov: fovValue(), focallength: $('focallength').checked, adsmult: $('adsmult').value.trim(), scopemult: $('scopemult').value.trim(), dxvk: $('dxvk').checked } });
});

// Links of the bottom rail: the Rust side knows the addresses and opens them in the player's browser.
const PREVIEW_LINKS = { status: 'https://grp.limacashout.pro/status/', source: 'https://github.com/Aquilabot/GRPLauncher', support: 'https://ko-fi.com/aquilabot' };
for (const b of document.querySelectorAll('.ext')) b.addEventListener('click', () => {
  if (tauri) invoke('open_link', { which: b.dataset.url }).catch(() => {});
  else window.open(PREVIEW_LINKS[b.dataset.url], '_blank', 'noopener');
});

// Release notes from the server, newest first. Their text goes in as text, never as HTML. Entries newer than the
// last one the player saw get a mark; touching the list or pressing Play counts as seen (the marks stay until the
// window closes, so the player can still tell which ones were new).
let notesTop = 0, notesSeen = 0, notesShown = [];

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

function showDate(d) {
  const [y, m, day] = d.split('-').map(Number), month = TEXT[lang].months[m - 1];
  return month ? (lang === 'en' ? `${month} ${day}, ${y}` : `${day} ${month} ${y}`) : d;
}

function drawNotes() {
  const notes = notesShown;
  $('notesEmpty').hidden = notes.length > 0;
  $('notesList').hidden = notes.length === 0;
  $('notesList').replaceChildren(...notes.map((n) => {
    const isNew = n.id > notesSeen;
    const a = el('article', isNew ? 'note new' : 'note');
    const meta = el('div', 'note-meta');
    if (n.date) meta.append(el('span', 'note-date', showDate(n.date)));
    if (n.tag) meta.append(el('span', 'note-tag', t(`tag.${n.tag}`)));
    if (isNew) meta.append(el('span', 'note-new', t('notes.fresh')));
    const body = el('div');
    body.append(el('h3', '', n.title));
    if (n.items.length) {
      const ul = el('ul');
      ul.append(...n.items.map((x) => el('li', '', x)));
      body.append(ul);
    }
    a.append(meta, body);
    return a;
  }));
  const count = notes.filter((n) => n.id > notesSeen).length;
  $('notesNew').hidden = count === 0 || notesTop <= notesMarked;
  $('notesNew').textContent = count === 1 ? t('notes.new1') : t('notes.newN', { n: count });
}
let notesMarked = 0;
fresh.push(drawNotes);

async function loadNotes() {
  const r = await invoke('get_notes').catch(() => null);
  notesShown = (r && r.notes) || [];
  if (notesShown.length) {
    notesTop = notesShown[0].id;
    notesSeen = Math.max(notesSeen, r.seen || 0);
    // A player who has never seen the list: everything in it is history, nothing is news.
    if (!notesSeen) { notesSeen = notesMarked = notesTop; invoke('mark_notes_seen', { id: notesTop }); }
  }
  drawNotes();
}

function markNotesSeen() {
  if (notesTop <= notesMarked || notesTop <= notesSeen) return;
  notesMarked = notesTop;
  $('notesNew').hidden = true;
  invoke('mark_notes_seen', { id: notesTop });
}

for (const ev of ['pointerdown', 'wheel', 'keydown']) $('notesList').addEventListener(ev, markNotesSeen, { passive: true });

// A list that goes on below its edge fades there, until the player reaches the end.
function watchMore(box) {
  const sync = () => box.classList.toggle('more', box.scrollHeight - box.scrollTop - box.clientHeight > 8);
  box.addEventListener('scroll', sync, { passive: true });
  new ResizeObserver(sync).observe(box);
  new MutationObserver(sync).observe(box, { childList: true, subtree: true, attributes: true, attributeFilter: ['hidden'] });
  sync();
}
watchMore($('notesList'));
watchMore(document.querySelector('.set-scroll'));
$('form').addEventListener('submit', markNotesSeen);

listen('status', (e) => setStatus(e.payload.text, e.payload.fraction));
listen('playing', () => { if (tauri) tauri.window.getCurrentWindow().minimize(); });
listen('done', (e) => {
  setBusy(false);
  setStatus(e.payload.error || '', -1, e.payload.error ? 'fault' : '');
  if (e.payload.need_invite) {
    $('inviteRow').hidden = false;
    $('invite').focus();
  } else {
    $('password').value = '';
    if (!e.payload.error) { $('modeLogin').checked = true; syncMode(); }
  }
  if (tauri) { const w = tauri.window.getCurrentWindow(); w.unminimize(); w.setFocus(); }
  loadNotes(); // the server may have changed while the player was in the game
  invoke('get_state').then((s) => showDxvk(s.dxvk)); // a game that failed on Vulkan turned it off
});

(async () => {
  drawTerrain();
  const s = await invoke('get_state');
  $('name').value = s.name;
  $('gamedir').value = s.gamedir;
  $('windowed').checked = s.windowed;
  if (LINUX) $('windowedRow').hidden = true;   // always a screen-sized window there
  $('server').textContent = s.server;
  fresh.push(() => { $('version').textContent = t('version', { v: s.version }); });
  fillResolutions(s.resolution);
  $('mousesens').value = s.mousesens || '';
  fillMaxFps(s.maxfps);
  $('focallength').checked = !!s.focallength;
  $('adsmult').value = s.adsmult || '';
  $('scopemult').value = s.scopemult || '';
  $('clientmods').checked = s.clientmods;
  setFov(s.fov);
  syncClientMods(false);
  showDxvk(s.dxvk);
  $('dxvk').disabled = LINUX;
  // The language the player chose; the first time, English for a system in English and Spanish for the rest.
  applyLang(s.lang || (/^en\b/i.test(navigator.language || '') ? 'en' : 'es'));
  (s.name ? $('password') : $('name')).focus();
  await loadNotes();
  if (!tauri) preview(location.hash.slice(1).split(','));
})();

// Outside the launcher only: states to look at, named in the address (index.html#settings,en,create,busy,calc,fault).
function preview(want) {
  if (want.includes('en')) applyLang('en');
  if (want.includes('settings')) showView('settings');
  if (want.includes('create')) { $('create').checked = true; syncMode(); }
  if (want.includes('calc')) { showSensCalc(true); $('sensInput').value = '30'; updateSensCalc(); }
  if (want.includes('fov')) { $('fov').value = 8; drawCone(); }
  if (want.includes('busy')) { setBusy(true); setStatus(lang === 'en' ? 'Downloading Yeti.big  (1840 of 4012 MB)' : 'Descargando Yeti.big  (1840 de 4012 MB)', 0.46); }
  if (want.includes('fault')) setStatus(lang === 'en' ? 'Wrong nickname or password.' : 'Nickname o contraseña incorrectos.', -1, 'fault');
  if (want.includes('reset')) showReset(true);
}
