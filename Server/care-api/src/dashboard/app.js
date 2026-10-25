// Healoo administrator dashboard (Documentation/Administration_Design.md section 8).
// Plain JavaScript, no libraries. Signs in with Auth0 (Authorization Code + PKCE) or, on a LAN
// trial server, with a test account, then reads /v1/admin/*. Every value from the server is put
// on the page with textContent, never as HTML.
'use strict';

const $ = (id) => document.getElementById(id);

function el(tag, attrs, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') e.className = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v === true ? '' : v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k instanceof Node ? k : String(k));
  return e;
}
const clear = (node) => { node.replaceChildren(); return node; };

const METRIC_LABELS = {
  users_registered: 'New sign-ups', accounts_created: 'Accounts added by administrators',
  items_created: 'Records created', bookings: 'Bookings (new appointments)', reports: 'Reports added',
  items_updated: 'Records edited', items_closed: 'Records closed', items_reopened: 'Records reopened',
  appointments: 'Appointment changes', reminders: 'Reminders set', uploads: 'Files uploaded',
  messages: 'Messages', grants_added: 'Records shared', grants_revoked: 'Sharing stopped',
  affiliations_ended: 'Doctors leaving hospitals', delegations: 'Assistant delegations',
  access_denied: 'Access refused', accounts_deactivated: 'Accounts deactivated',
  accounts_reactivated: 'Accounts reactivated', hospitals_created: 'Hospitals added',
  admins_appointed: 'Administrators appointed', admins_removed: 'Administrators removed',
  invites_created: 'Invite codes created',
};
const ROLE_LABELS = {
  PATIENT: 'Patient', DOCTOR: 'Doctor', ASSISTANT: 'Assistant', LAB: 'Lab', HOSPITAL: 'Hospital',
  ADMINISTRATOR: 'Hospital administrator', APP_ADMINISTRATOR: 'Platform administrator',
};
const CATEGORY = { RECORDS: ['Records', ''], SHARING: ['Sharing', 'sand'], ACCOUNTS: ['Accounts', 'grey'] };
const nf = new Intl.NumberFormat();
const fmt = (n) => (n == null ? '—' : nf.format(n));

function bytes(n) {
  if (n == null) return '—';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n < 10 && i ? n.toFixed(1) : Math.round(n)} ${u[i]}`;
}
const utcToday = () => new Date().toISOString().slice(0, 10);

// ---------------- sign-in ----------------

const TOKEN_KEY = 'healoo_admin_token';
const PKCE_KEY = 'healoo_admin_pkce';
const auth = { cfg: null, token: null };

function saveToken(token, expiresIn) {
  auth.token = token;
  try { sessionStorage.setItem(TOKEN_KEY, JSON.stringify({ token, exp: Date.now() + (expiresIn || 3600) * 1000 })); } catch (_) { /* private mode */ }
}
function loadToken() {
  try {
    const t = JSON.parse(sessionStorage.getItem(TOKEN_KEY) || 'null');
    if (t && t.exp > Date.now() + 30000) return t.token;
  } catch (_) { /* ignore */ }
  return null;
}
function forgetToken() {
  auth.token = null;
  try { sessionStorage.removeItem(TOKEN_KEY); } catch (_) { /* ignore */ }
}

const b64url = (buf) => btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const redirectUri = () => `${location.origin}/admin/`;

async function auth0Login() {
  const c = auth.cfg;
  const verifier = b64url(crypto.getRandomValues(new Uint8Array(32)));
  const challenge = b64url(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier)));
  const state = b64url(crypto.getRandomValues(new Uint8Array(16)));
  sessionStorage.setItem(PKCE_KEY, JSON.stringify({ verifier, state }));
  const u = new URL(`https://${c.auth0_domain}/authorize`);
  u.search = new URLSearchParams({
    response_type: 'code', client_id: c.client_id, redirect_uri: redirectUri(), audience: c.audience,
    scope: 'openid profile email', code_challenge: challenge, code_challenge_method: 'S256', state,
    // Always show Auth0's login page, even when Auth0 still remembers a login: after a refused
    // account the administrator can pick another one instead of being sent straight back.
    prompt: 'login',
  }).toString();
  location.assign(u.toString());
}

async function auth0Callback(params) {
  const saved = JSON.parse(sessionStorage.getItem(PKCE_KEY) || 'null');
  sessionStorage.removeItem(PKCE_KEY);
  history.replaceState(null, '', location.pathname + location.hash);
  if (params.get('error')) throw new Error(params.get('error_description') || params.get('error'));
  if (!saved || saved.state !== params.get('state')) throw new Error('Sign-in was interrupted. Try again.');
  const r = await fetch(`https://${auth.cfg.auth0_domain}/oauth/token`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ grant_type: 'authorization_code', client_id: auth.cfg.client_id, code_verifier: saved.verifier, code: params.get('code'), redirect_uri: redirectUri() }),
  });
  const body = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(body.error_description || 'Auth0 refused the sign-in');
  saveToken(body.access_token, body.expires_in);
}

async function devAccounts() {
  const box = clear($('dev-accounts'));
  const r = await fetch('/dev/users');
  const list = (await r.json()).data || [];
  list.sort((a, b) => (b.role === 'APP_ADMINISTRATOR') - (a.role === 'APP_ADMINISTRATOR'));
  for (const u of list) {
    box.append(el('button', {
      type: 'button', class: u.role === 'APP_ADMINISTRATOR' ? '' : 'secondary',
      onclick: async () => {
        const t = await fetch('/dev/token', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ public_id: u.public_id }) });
        const b = await t.json();
        if (!t.ok) return signInMessage(b.error?.message || 'Could not sign in', true);
        saveToken(b.access_token, b.expires_in);
        enter();
      },
    }, el('span', {}, u.display_name), el('span', { class: 'small' }, `${ROLE_LABELS[u.role] || u.role} · ${u.public_id}`)));
  }
}

function signInMessage(text, error) {
  const m = $('signin-message');
  m.textContent = text;
  m.className = `notice${error ? ' error' : ''}`;
  m.classList.toggle('hidden', !text);
}

function showSignIn(message, error) {
  $('app').classList.add('hidden');
  $('signin').classList.remove('hidden');
  signInMessage(message || '', error);
  const dev = auth.cfg.auth_mode === 'dev';
  $('signin-dev').classList.toggle('hidden', !dev);
  $('signin-auth0').classList.toggle('hidden', dev);
  if (dev) devAccounts().catch((e) => signInMessage(e.message, true));
  else if (!auth.cfg.client_id) signInMessage('The server has no ADMIN_AUTH0_CLIENT_ID set, so the dashboard cannot sign in with Auth0.', true);
}

function signOut() {
  forgetToken();
  if (auth.cfg.auth_mode === 'auth0' && auth.cfg.client_id) {
    const u = new URL(`https://${auth.cfg.auth0_domain}/v2/logout`);
    u.search = new URLSearchParams({ client_id: auth.cfg.client_id, returnTo: redirectUri() }).toString();
    location.assign(u.toString());
  } else {
    showSignIn('Signed out.');
  }
}

// ---------------- API ----------------

async function api(path, opts = {}) {
  const headers = { Authorization: `Bearer ${auth.token}` };
  if (opts.body) headers['Content-Type'] = 'application/json';
  const r = await fetch(`/v1${path}`, { ...opts, headers });
  if (r.status === 401) { forgetToken(); showSignIn('Your session ended. Sign in again.'); throw new Error('signed out'); }
  const body = r.status === 204 ? null : await r.json().catch(() => null);
  if (!r.ok) throw new Error(body?.error?.message || `The server answered ${r.status}`);
  return body;
}

function showError(e) {
  if (e?.message === 'signed out') return;
  const box = $('error');
  box.textContent = e ? e.message : '';
  box.classList.toggle('hidden', !e);
}

// ---------------- app shell ----------------

let me = null;

async function enter() {
  try {
    me = await api('/me');
  } catch (e) {
    if (e.message !== 'signed out') showSignIn(e.message, true);
    return;
  }
  if (!me.roles.includes('APP_ADMINISTRATOR')) {
    forgetToken();
    showSignIn(`${me.display_name} (${me.public_id}) is not a platform administrator. On the server, run: docker compose run --rm care-api care-admin grant ${me.public_id} — or choose "Use a different account".`, true);
    return;
  }
  $('who').textContent = `${me.display_name} · ${me.public_id}`;
  $('signin').classList.add('hidden');
  $('app').classList.remove('hidden');
  route();
}

const PAGES = { overview: loadOverview, usage: loadUsage, accounts: loadAccounts, activity: loadActivity, hospitals: loadHospitals };

function route() {
  const page = PAGES[location.hash.slice(1)] ? location.hash.slice(1) : 'overview';
  for (const b of document.querySelectorAll('nav.tabs button')) b.setAttribute('aria-selected', String(b.dataset.page === page));
  for (const name of Object.keys(PAGES)) $(`page-${name}`).hidden = name !== page;
  showError(null);
  PAGES[page]().catch(showError);
}

// ---------------- overview ----------------

let overview = null;

function stat(label, value, accent) {
  return el('div', { class: `stat${accent ? ' accent' : ''}` }, el('div', { class: 'n' }, value), el('div', { class: 'l' }, label));
}

async function loadOverview() {
  overview = await api('/admin/overview?days=14');
  const t = overview.today;
  const a = overview.accounts;
  // One row at any width: five fixed figures and a sixth chosen from a dropdown.
  const more = [
    ['reports', 'Reports today', fmt(t.reports)],
    ['uploads', 'Files uploaded today', fmt(t.uploads)],
    ['items_created', 'Records created today', fmt(t.items_created)],
    ['grants_added', 'Records shared today', fmt(t.grants_added)],
    ['appointments', 'Appointment changes today', fmt(t.appointments)],
    ['access_denied', 'Access refused today', fmt(t.access_denied)],
    ['never_signed_in', 'Never signed in', fmt(a.never_signed_in)],
    ['active', 'Active accounts', fmt(a.active)],
    ['deactivated', 'Deactivated accounts', fmt(a.deactivated)],
    ...Object.entries(a.by_role).map(([r, n]) => [`role_${r}`, `${ROLE_LABELS[r] || r}s`, fmt(n)]),
  ];
  let picked = 'reports';
  try { picked = localStorage.getItem('healoo_admin_more') || picked; } catch (_) { /* private mode */ }
  if (!more.some(([k]) => k === picked)) picked = more[0][0];
  const value = el('div', { class: 'n' });
  const select = el('select', { 'aria-label': 'Choose another figure', onchange: () => {
    show(select.value);
    try { localStorage.setItem('healoo_admin_more', select.value); } catch (_) { /* ignore */ }
  } }, ...more.map(([k, label]) => el('option', { value: k }, label)));
  const show = (k) => { const m = more.find(([key]) => key === k); value.textContent = m ? m[2] : '—'; select.value = k; };
  show(picked);
  clear($('ov-stats')).append(
    stat('Accounts', fmt(a.total), true),
    stat('Active users today', fmt(t.active_users), true),
    stat('New sign-ups today', fmt(t.users_registered)),
    stat('Messages today', fmt(t.messages)),
    stat('Bookings today', fmt(t.bookings)),
    el('div', { class: 'stat more' }, value, select),
  );

  const sel = $('ov-metric');
  if (!sel.options.length) {
    for (const m of overview.metrics) sel.append(el('option', { value: m }, METRIC_LABELS[m] || m));
    sel.value = 'messages';
    sel.addEventListener('change', drawTrend);
  }
  drawTrend();

  const roles = clear($('ov-roles'));
  const max = Math.max(1, ...Object.values(a.by_role));
  const tbody = el('tbody');
  for (const [role, n] of Object.entries(a.by_role).sort((x, y) => y[1] - x[1])) {
    tbody.append(el('tr', {},
      el('td', {}, ROLE_LABELS[role] || role),
      el('td', { class: 'num' }, fmt(n)),
      el('td', {}, el('div', { class: 'meter' }, el('span', { 'data-w': n / max })))));
  }
  roles.append(el('table', {}, tbody), el('p', { class: 'small muted' }, `${fmt(a.active)} active, ${fmt(a.deactivated)} deactivated.`));
  for (const s of roles.querySelectorAll('.meter span')) s.style.width = `${Math.round(Number(s.dataset.w) * 100)}%`;

  const list = clear($('ov-recent'));
  if (!overview.recent.length) list.append(el('li', {}, el('span'), el('span', { class: 'muted' }, 'Nothing yet today.')));
  for (const r of overview.recent) list.append(activityLine(r));
}

function activityLine(r) {
  const time = new Date(r.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const whose = r.subject_name && r.subject_name !== r.actor_name ? ` (${r.subject_name})` : '';
  return el('li', {}, el('time', { datetime: r.at }, time), el('span', {}, el('strong', {}, r.actor_name || 'Someone'), ` ${r.detail}${whose}`));
}

function drawTrend() {
  const metric = $('ov-metric').value;
  $('ov-legend').textContent = METRIC_LABELS[metric] || metric;
  const rows = overview.trend;
  const W = 760, H = 220, pad = { l: 34, r: 8, t: 10, b: 26 };
  const max = Math.max(1, ...rows.map((r) => Math.max(r[metric], r.active_users)));
  const step = (W - pad.l - pad.r) / rows.length;
  const y = (v) => H - pad.b - (v / max) * (H - pad.t - pad.b);
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', `0 0 ${W} ${H}`);
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', `${METRIC_LABELS[metric]} and active users per day`);
  const add = (name, attrs, text) => {
    const n = document.createElementNS(NS, name);
    for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);
    if (text != null) n.textContent = text;
    svg.append(n);
    return n;
  };
  for (const v of [0, Math.round(max / 2), max]) {
    add('line', { class: 'axis', x1: pad.l, x2: W - pad.r, y1: y(v), y2: y(v) });
    add('text', { x: pad.l - 6, y: y(v) + 4, 'text-anchor': 'end' }, nf.format(v));
  }
  rows.forEach((r, i) => {
    const x = pad.l + i * step;
    add('rect', { class: 'bar alt', x: x + step * 0.12, width: step * 0.76, y: y(r.active_users), height: H - pad.b - y(r.active_users) });
    const bar = add('rect', { class: 'bar', x: x + step * 0.28, width: step * 0.44, y: y(r[metric]), height: H - pad.b - y(r[metric]) });
    const tip = document.createElementNS(NS, 'title');
    tip.textContent = `${r.day}: ${nf.format(r[metric])} ${METRIC_LABELS[metric].toLowerCase()}, ${nf.format(r.active_users)} active users`;
    bar.append(tip);
    if (i % 2 === rows.length % 2 || rows.length <= 8) add('text', { x: x + step / 2, y: H - 8, 'text-anchor': 'middle' }, r.day.slice(5));
  });
  clear($('ov-chart')).append(svg);
}

// ---------------- usage ----------------

let usage = null;
let usagePeriod = 'week';
const USAGE_HEADLINE = ['messages', 'reports', 'uploads', 'bookings', 'users_registered', 'items_created', 'grants_added'];

async function loadUsage() {
  const box = $('us-stats');
  if (!usage) clear(box).append(el('p', { class: 'muted' }, 'Adding up a year of activity…'));
  usage = await api('/admin/usage');
  for (const b of document.querySelectorAll('#us-period button')) {
    b.onclick = () => { usagePeriod = b.dataset.period; drawUsage(); };
  }
  drawUsage();
}

function drawUsage() {
  for (const b of document.querySelectorAll('#us-period button')) b.setAttribute('aria-pressed', String(b.dataset.period === usagePeriod));
  const p = usage.periods.find((x) => x.period === usagePeriod);
  $('us-caption').textContent = p.days === 1 ? 'Today (UTC).' : `The last ${p.days} days, today included.`;
  clear($('us-stats')).append(
    stat(`Active users (of ${fmt(usage.users_total)})`, fmt(p.active_users), true),
    stat(`Active hospitals (of ${fmt(usage.hospitals_total)})`, fmt(p.active_hospitals), true),
    ...USAGE_HEADLINE.map((m) => stat(METRIC_LABELS[m], fmt(p.totals[m]))),
  );

  const head = el('tr', {}, el('th', {}, 'Over the last'), ...usage.periods.map((x) => el('th', { class: 'num' }, x.days === 1 ? 'Day' : `${x.days} days`)));
  const body = el('tbody');
  const row = (label, f) => body.append(el('tr', {}, el('td', {}, label), ...usage.periods.map((x) => el('td', { class: 'num' }, fmt(f(x))))));
  row('Active users', (x) => x.active_users);
  row('Active hospitals', (x) => x.active_hospitals);
  for (const m of Object.keys(METRIC_LABELS)) {
    if (usage.periods.some((x) => x.totals[m])) row(METRIC_LABELS[m], (x) => x.totals[m]);
  }
  clear($('us-table')).append(el('thead', {}, head), body);

  const s = usage.storage;
  clear($('us-storage')).append(
    stat('Database on disk', s.database_mib ? `${fmt(s.database_mib)} MiB` : 'under 1 MiB', true),
    stat('Files stored', fmt(s.files.count)),
    stat('File storage', bytes(s.files.bytes)),
  );
  const tb = el('tbody');
  const shown = s.tables.filter((t) => t.mib > 0);
  for (const t of shown) tb.append(el('tr', {}, el('td', {}, t.table), el('td', { class: 'num' }, `${fmt(t.mib)} MiB`)));
  if (shown.length < s.tables.length) tb.append(el('tr', {}, el('td', { class: 'muted' }, `${s.tables.length - shown.length} smaller tables`), el('td', { class: 'num muted' }, '< 1 MiB each')));
  clear($('us-tables')).append(el('thead', {}, el('tr', {}, el('th', {}, 'Table'), el('th', { class: 'num' }, 'Size'))), tb);

  const names = { records: 'Records (items)', messages: 'Messages', files: 'Files attached', appointments: 'Appointments', vital_readings: 'Vital readings' };
  const rb = el('tbody');
  rb.append(el('tr', {}, el('td', {}, 'Accounts'), el('td', { class: 'num' }, fmt(usage.users_total))));
  for (const [k, v] of Object.entries(s.records)) rb.append(el('tr', {}, el('td', {}, names[k] || k), el('td', { class: 'num' }, v == null ? 'too many to count quickly' : fmt(v))));
  clear($('us-records')).append(el('thead', {}, el('tr', {}, el('th', {}, 'What'), el('th', { class: 'num' }, 'How many'))), rb);
}

// ---------------- accounts ----------------

const accountsState = { offset: 0, limit: 50, total: 0 };
let searchTimer = null;

async function loadAccounts() {
  const q = new URLSearchParams({ q: $('ac-q').value, role: $('ac-role').value, status: $('ac-status').value, offset: accountsState.offset, limit: accountsState.limit });
  const r = await api(`/admin/accounts?${q}`);
  accountsState.total = r.total;
  $('ac-total').textContent = `${fmt(r.total)} account${r.total === 1 ? '' : 's'}`;
  const tb = el('tbody');
  for (const a of r.data) {
    const status = !a.active ? el('span', { class: 'pill clay' }, 'Deactivated')
      : a.signed_in ? el('span', { class: 'pill' }, 'Active') : el('span', { class: 'pill grey' }, 'Never signed in');
    tb.append(el('tr', {},
      el('td', {}, a.display_name),
      el('td', {}, a.public_id),
      el('td', {}, el('span', { class: `pill ${a.role === 'PATIENT' ? 'grey' : a.role.includes('ADMIN') ? 'sand' : ''}` }, ROLE_LABELS[a.role] || a.role)),
      el('td', {}, a.hospital_name || ''),
      el('td', { class: 'small' }, a.email || ''),
      el('td', {}, status),
      el('td', { class: 'small muted' }, a.created_at ? new Date(a.created_at).toLocaleDateString() : '')));
  }
  if (!r.data.length) tb.append(el('tr', {}, el('td', { colspan: 7, class: 'muted' }, 'No accounts match.')));
  clear($('ac-table')).append(el('thead', {}, el('tr', {}, ...['Name', 'Healoo ID', 'Role', 'Hospital', 'Email', 'Status', 'Joined'].map((h) => el('th', {}, h)))), tb);
  const last = Math.min(accountsState.offset + accountsState.limit, r.total);
  $('ac-page').textContent = r.total ? `${accountsState.offset + 1}–${last} of ${fmt(r.total)}` : '';
  $('ac-prev').disabled = accountsState.offset === 0;
  $('ac-next').disabled = last >= r.total;
}

// ---------------- activity ----------------

async function loadActivity() {
  if (!$('at-day').value) { $('at-day').value = utcToday(); $('at-day').max = utcToday(); }
  const q = new URLSearchParams({ day: $('at-day').value, category: $('at-category').value });
  const r = await api(`/admin/activity?${q}`);
  $('at-count').textContent = `${fmt(r.data.length)} event${r.data.length === 1 ? '' : 's'}`;
  const tb = el('tbody');
  for (const a of r.data) {
    const [label, cls] = CATEGORY[a.category] || [a.category, 'grey'];
    tb.append(el('tr', {},
      el('td', { class: 'small muted' }, new Date(a.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })),
      el('td', {}, a.actor_name || 'Someone'),
      el('td', {}, a.detail),
      el('td', {}, a.subject_name && a.subject_name !== a.actor_name ? a.subject_name : ''),
      el('td', {}, el('span', { class: `pill ${cls}` }, label))));
  }
  if (!r.data.length) tb.append(el('tr', {}, el('td', { colspan: 5, class: 'muted' }, 'Nothing happened that day.')));
  clear($('at-table')).append(el('thead', {}, el('tr', {}, ...['Time', 'Who', 'What', 'Whose record / account', 'Kind'].map((h) => el('th', {}, h)))), tb);
}

// ---------------- hospitals ----------------

async function loadHospitals() {
  const r = await api('/admin/hospitals');
  const box = clear($('hs-list'));
  if (!r.data.length) box.append(el('div', { class: 'card muted' }, 'No hospitals yet.'));
  for (const h of r.data) box.append(hospitalCard(h, r.max_admins_per_hospital));
}

function hospitalCard(h, maxAdmins) {
  const dl = el('dl', {});
  const row = (k, v) => { if (v) dl.append(el('dt', {}, k), el('dd', {}, v)); };
  row('Healoo ID', h.public_id);
  row('Address', h.location);
  row('Phone', h.phone);
  row('Registration', h.official_number);
  row('Doctors', fmt(h.doctors));

  const admins = el('div', {});
  if (!h.administrators.length) admins.append(el('p', { class: 'small muted' }, 'No administrator yet.'));
  for (const a of h.administrators) {
    admins.append(el('div', { class: 'filters' },
      el('span', {}, `${a.display_name} · ${a.public_id}${a.email ? ` · ${a.email}` : ''}`),
      a.active ? null : el('span', { class: 'pill clay' }, 'Deactivated'),
      el('button', { type: 'button', class: 'link', onclick: async () => {
        if (!confirm(`Stop ${a.display_name} being ${h.display_name}'s administrator?`)) return;
        try { await api(`/admin/hospitals/${h.hospital_id}/administrators/${a.user_id}`, { method: 'DELETE' }); loadHospitals(); } catch (e) { showError(e); }
      } }, 'Remove')));
  }
  const input = el('input', { placeholder: 'Healoo ID, e.g. HL-4K7Q2', 'aria-label': `Healoo ID of ${h.display_name}'s new administrator`, maxlength: 12 });
  const msg = el('span', { class: 'small' });
  const appoint = el('form', { class: 'filters mt', onsubmit: async (ev) => {
    ev.preventDefault();
    msg.textContent = '';
    try {
      const a = await api(`/admin/hospitals/${h.hospital_id}/administrators`, { method: 'POST', body: JSON.stringify({ public_id: input.value.trim() }) });
      msg.textContent = `${a.display_name} is now an administrator.`;
      loadHospitals();
    } catch (e) { msg.textContent = e.message; msg.className = 'small notice error'; }
  } }, input, el('button', { type: 'submit', class: 'secondary' }, 'Appoint administrator'), msg);

  return el('div', { class: 'card hospital' },
    el('div', { class: 'card-head' }, el('h2', {}, h.display_name), h.headline ? el('span', { class: 'muted' }, h.headline) : null),
    dl, el('h3', {}, maxAdmins ? `Administrators · ${h.administrators.length} of ${maxAdmins}` : 'Administrators'), admins, appoint, invitesSection(h),
    el('p', { class: 'tiny muted' }, 'The person signs up in the Healoo app first; their Healoo ID is on their profile. The hospital\'s details are edited by its administrator in the app (Settings → Administration).'));
}

// ---- invite codes (Administration_Design.md section 11) ----

const INVITE_STATUS = { PENDING: ['Waiting', ''], USED: ['Used', 'grey'], REVOKED: ['Revoked', 'clay'], EXPIRED: ['Expired', 'sand'] };

function invitesSection(h) {
  const list = el('div', { class: 'invites' });
  const shown = el('div', {});
  const note = el('input', { placeholder: 'Who is it for? (optional)', maxlength: 80, 'aria-label': `Who the invite for ${h.display_name} is for` });
  const msg = el('span', { class: 'small' });

  async function refresh() {
    try {
      const r = await api(`/admin/hospitals/${h.hospital_id}/invites`);
      clear(list);
      if (!r.data.length) list.append(el('p', { class: 'small muted' }, 'No invite codes yet.'));
      for (const i of r.data) {
        const [label, cls] = INVITE_STATUS[i.status] || [i.status, 'grey'];
        const when = i.status === 'USED' && i.used_by ? `by ${i.used_by.display_name} (${i.used_by.public_id}) on ${new Date(i.used_at).toLocaleDateString()}`
          : i.status === 'PENDING' ? `expires ${new Date(i.expires_at).toLocaleString()}` : `created ${new Date(i.created_at).toLocaleDateString()}`;
        list.append(el('div', { class: 'filters invite-row' },
          el('span', { class: `pill ${cls}` }, label),
          el('span', {}, i.note || 'Invite'),
          el('span', { class: 'small muted' }, when),
          i.status === 'PENDING' ? el('button', { type: 'button', class: 'link', onclick: async () => {
            if (!confirm('Revoke this invite code? It will stop working.')) return;
            try { await api(`/admin/hospitals/${h.hospital_id}/invites/${i.invite_id}`, { method: 'DELETE' }); refresh(); } catch (e) { showError(e); }
          } }, 'Revoke') : null));
      }
    } catch (e) { clear(list).append(el('p', { class: 'small notice error' }, e.message)); }
  }

  const form = el('form', { class: 'filters mt', onsubmit: async (ev) => {
    ev.preventDefault();
    msg.textContent = '';
    try {
      const r = await api(`/admin/hospitals/${h.hospital_id}/invites`, { method: 'POST', body: JSON.stringify({ note: note.value }) });
      note.value = '';
      const code = el('code', { class: 'invite-code' }, r.code);
      const copy = el('button', { type: 'button', class: 'secondary', onclick: async () => {
        try { await navigator.clipboard.writeText(r.code); copy.textContent = 'Copied'; } catch (_) { copy.textContent = 'Select and copy it'; }
      } }, 'Copy');
      clear(shown).append(el('div', { class: 'notice ok mt' },
        el('div', { class: 'filters' }, code, copy),
        el('p', { class: 'small' }, `Send this code to the new administrator. It works once, until ${new Date(r.expires_at).toLocaleString()}. ` +
          'They sign in to the Healoo app, then go to Settings → "I have an invite code". You won\'t see the code again.')));
      refresh();
    } catch (e) { msg.textContent = e.message; msg.className = 'small notice error'; }
  } }, note, el('button', { type: 'submit', class: 'secondary' }, 'Create invite code'), msg);

  refresh();
  return el('div', {}, el('h3', { class: 'mt' }, 'Invite codes'),
    el('p', { class: 'tiny muted' }, `A code makes whoever enters it in the app this hospital's administrator. Single use, valid for 7 days.`),
    form, shown, list);
}

async function addHospital(ev) {
  ev.preventDefault();
  const f = ev.target;
  const msg = $('hs-form-msg');
  const body = Object.fromEntries(new FormData(f).entries());
  for (const k of Object.keys(body)) if (!body[k].trim()) delete body[k];
  try {
    const r = await api('/admin/hospitals', { method: 'POST', body: JSON.stringify(body) });
    msg.className = 'small notice ok';
    msg.textContent = `${r.hospital.display_name} added (${r.hospital.public_id})${r.administrator ? `, with administrator ${r.administrator.display_name}` : ''}.`;
    f.reset();
    loadHospitals();
  } catch (e) { msg.className = 'small notice error'; msg.textContent = e.message; }
}

// ---------------- start ----------------

function wire() {
  $('btn-login').addEventListener('click', () => auth0Login().catch((e) => signInMessage(e.message, true)));
  // Signs out of Auth0 too (it remembers the last login), then comes back to this page.
  $('btn-switch').addEventListener('click', signOut);
  $('btn-logout').addEventListener('click', signOut);
  for (const b of document.querySelectorAll('nav.tabs button')) b.addEventListener('click', () => { location.hash = b.dataset.page; });
  window.addEventListener('hashchange', route);
  $('ac-q').addEventListener('input', () => { clearTimeout(searchTimer); searchTimer = setTimeout(() => { accountsState.offset = 0; loadAccounts().catch(showError); }, 300); });
  for (const id of ['ac-role', 'ac-status']) $(id).addEventListener('change', () => { accountsState.offset = 0; loadAccounts().catch(showError); });
  $('ac-prev').addEventListener('click', () => { accountsState.offset = Math.max(0, accountsState.offset - accountsState.limit); loadAccounts().catch(showError); });
  $('ac-next').addEventListener('click', () => { accountsState.offset += accountsState.limit; loadAccounts().catch(showError); });
  for (const id of ['at-day', 'at-category']) $(id).addEventListener('change', () => loadActivity().catch(showError));
  $('hs-form').addEventListener('submit', addHospital);
}

async function start() {
  wire();
  try {
    auth.cfg = await (await fetch('/admin/config.json')).json();
  } catch (e) {
    document.body.textContent = 'The dashboard could not reach the Healoo server.';
    return;
  }
  const params = new URLSearchParams(location.search);
  if (auth.cfg.auth_mode === 'auth0' && (params.has('code') || params.has('error'))) {
    try { await auth0Callback(params); } catch (e) { return showSignIn(e.message, true); }
  }
  auth.token = loadToken();
  if (auth.token) enter(); else showSignIn();
}

start();
