pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Nostr Relay</title>
<style>
  :root {
    --bg:        #0d0d14;
    --surface:   #16161f;
    --surface2:  #1e1e2e;
    --border:    #2a2a3d;
    --purple:    #7c3aed;
    --purple-lt: #9d6ef5;
    --purple-dk: #5b21b6;
    --text:      #e2e2f0;
    --muted:     #888aaa;
    --green:     #22c55e;
    --red:       #ef4444;
    --yellow:    #f59e0b;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    background: var(--bg);
    color: var(--text);
    min-height: 100vh;
    padding-bottom: 3rem;
  }

  /* ── header ── */
  header {
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    padding: 1rem 2rem;
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .logo {
    width: 38px; height: 38px;
    background: linear-gradient(135deg, var(--purple-dk), var(--purple-lt));
    border-radius: 10px;
    display: flex; align-items: center; justify-content: center;
    font-size: 1.2rem;
  }
  .header-info h1 { font-size: 1.1rem; font-weight: 600; }
  .header-info p  { font-size: 0.78rem; color: var(--muted); }
  .relay-url {
    margin-left: auto;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4rem 0.8rem;
    font-size: 0.78rem;
    color: var(--purple-lt);
    font-family: monospace;
    cursor: pointer;
    transition: background 0.15s;
    display: flex; align-items: center; gap: 0.5rem;
  }
  .relay-url:hover { background: var(--border); }

  /* ── main ── */
  main { max-width: 960px; margin: 0 auto; padding: 2rem 1.5rem; }

  /* ── stats ── */
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
  }
  .stat-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1.2rem 1.4rem;
  }
  .stat-card .label {
    font-size: 0.73rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 0.5rem;
  }
  .stat-card .value {
    font-size: 2rem;
    font-weight: 700;
    line-height: 1;
  }
  .stat-card .value.purple { color: var(--purple-lt); }
  .stat-card .value.green  { color: var(--green); }

  /* ── section ── */
  .section {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
    margin-bottom: 1.5rem;
  }
  .section-header {
    padding: 1rem 1.4rem;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .section-header h2 { font-size: 0.95rem; font-weight: 600; }
  .section-body { padding: 1.4rem; }

  /* ── add form ── */
  .add-form { display: flex; gap: 0.75rem; flex-wrap: wrap; }
  .add-form input {
    flex: 1;
    min-width: 260px;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.55rem 0.9rem;
    color: var(--text);
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.15s;
  }
  .add-form input:focus { border-color: var(--purple); }
  .add-form input::placeholder { color: var(--muted); }

  /* ── buttons ── */
  .btn {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.55rem 1.1rem;
    border-radius: 8px;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: opacity 0.15s, transform 0.1s;
  }
  .btn:active { transform: scale(0.97); }
  .btn-primary { background: var(--purple); color: #fff; }
  .btn-primary:hover { opacity: 0.88; }
  .btn-danger  { background: transparent; color: var(--red); border: 1px solid var(--red); padding: 0.3rem 0.7rem; font-size: 0.78rem; }
  .btn-danger:hover  { background: var(--red); color: #fff; }

  /* ── table ── */
  .wl-table { width: 100%; border-collapse: collapse; }
  .wl-table th {
    text-align: left;
    font-size: 0.72rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0 0 0.7rem;
  }
  .wl-table td {
    padding: 0.7rem 0;
    border-top: 1px solid var(--border);
    font-size: 0.84rem;
    vertical-align: middle;
  }
  .wl-table tr:first-child td { border-top: none; }
  .npub { font-family: monospace; font-size: 0.78rem; color: var(--purple-lt); }
  .note-text { color: var(--muted); font-size: 0.8rem; }
  .date-text { color: var(--muted); font-size: 0.78rem; white-space: nowrap; }

  .empty-state {
    text-align: center;
    padding: 2.5rem 1rem;
    color: var(--muted);
    font-size: 0.88rem;
  }
  .empty-state svg { display: block; margin: 0 auto 0.75rem; opacity: 0.4; }

  /* ── toast ── */
  #toast {
    position: fixed; bottom: 1.5rem; right: 1.5rem;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem 1.1rem;
    font-size: 0.84rem;
    opacity: 0;
    transform: translateY(8px);
    transition: opacity 0.2s, transform 0.2s;
    pointer-events: none;
    z-index: 100;
  }
  #toast.show { opacity: 1; transform: translateY(0); }
  #toast.success { border-left: 3px solid var(--green); }
  #toast.error   { border-left: 3px solid var(--red); }

  /* ── status dot ── */
  .dot {
    display: inline-block;
    width: 8px; height: 8px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    margin-right: 0.4rem;
  }

  /* ── open/relay mode badge ── */
  .badge {
    font-size: 0.7rem;
    padding: 0.2rem 0.55rem;
    border-radius: 20px;
    font-weight: 500;
  }
  .badge-open     { background: #14532d; color: var(--green); }
  .badge-closed   { background: #450a0a; color: var(--red); }
  .badge-whitelisted { background: #3b0764; color: var(--purple-lt); }
</style>
</head>
<body>

<header>
  <div class="logo">⚡</div>
  <div class="header-info">
    <h1 id="relay-name">Nostr Relay</h1>
    <p id="relay-description">Loading…</p>
  </div>
  <div class="relay-url" id="relay-url-btn" onclick="copyUrl()">
    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none"
         stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
      <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
    </svg>
    <span id="ws-url">ws://loading…</span>
  </div>
</header>

<main>
  <div class="stats-grid">
    <div class="stat-card">
      <div class="label"><span class="dot"></span>Status</div>
      <div class="value green" style="font-size:1.1rem; margin-top:0.3rem">Online</div>
    </div>
    <div class="stat-card">
      <div class="label">Events Stored</div>
      <div class="value purple" id="stat-events">—</div>
    </div>
    <div class="stat-card">
      <div class="label">Active Connections</div>
      <div class="value purple" id="stat-conns">—</div>
    </div>
    <div class="stat-card">
      <div class="label">Whitelist Mode</div>
      <div class="value" style="font-size:1rem; margin-top:0.4rem" id="stat-mode">—</div>
    </div>
  </div>

  <div class="section">
    <div class="section-header">
      <h2>Whitelisted Pubkeys</h2>
      <span class="badge badge-whitelisted" id="wl-badge">0 keys</span>
    </div>
    <div class="section-body">
      <form class="add-form" id="add-form" onsubmit="addKey(event)">
        <input id="inp-npub" type="text" placeholder="npub1… or 64-char hex pubkey" autocomplete="off" />
        <input id="inp-note" type="text" placeholder="Note (optional)" style="max-width:200px" />
        <button type="submit" class="btn btn-primary">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
               fill="none" stroke="currentColor" stroke-width="2.5"
               stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          Add Key
        </button>
      </form>
    </div>

    <div style="padding: 0 1.4rem 1.4rem" id="wl-container">
      <div class="empty-state" id="wl-empty">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24"
             fill="none" stroke="currentColor" stroke-width="1.5"
             stroke-linecap="round" stroke-linejoin="round">
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
          <circle cx="9" cy="7" r="4"/>
          <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
          <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
        </svg>
        No keys whitelisted. The relay accepts all pubkeys in open mode.
      </div>
      <table class="wl-table" id="wl-table" style="display:none">
        <thead>
          <tr>
            <th>NPUB</th>
            <th>NOTE</th>
            <th>ADDED</th>
            <th></th>
          </tr>
        </thead>
        <tbody id="wl-body"></tbody>
      </table>
    </div>
  </div>
</main>

<div id="toast"></div>

<script>
const BASE = '';

function relayWsUrl() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${location.host}`;
}

function toast(msg, type = 'success') {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.className = `show ${type}`;
  setTimeout(() => el.className = '', 2500);
}

function copyUrl() {
  const url = relayWsUrl();
  navigator.clipboard.writeText(url).then(() => toast('Relay URL copied!'));
}

function formatDate(ts) {
  return new Date(ts * 1000).toLocaleDateString(undefined, {
    month: 'short', day: 'numeric', year: 'numeric'
  });
}

function npubShort(npub) {
  if (npub.length > 20) return npub.slice(0, 12) + '…' + npub.slice(-8);
  return npub;
}

async function loadStats() {
  try {
    const r = await fetch(`${BASE}/api/stats`);
    const d = await r.json();
    document.getElementById('stat-events').textContent = d.total_events.toLocaleString();
    document.getElementById('stat-conns').textContent = d.active_connections;
    document.getElementById('relay-name').textContent = d.relay_name || 'Nostr Relay';
    document.getElementById('relay-description').textContent = d.relay_description || '';
    const modeEl = document.getElementById('stat-mode');
    if (d.whitelist_count === 0) {
      modeEl.innerHTML = '<span class="badge badge-open">Open</span>';
    } else {
      modeEl.innerHTML = `<span class="badge badge-whitelisted">Whitelisted</span>`;
    }
  } catch(e) { console.error('stats error', e); }
}

async function loadWhitelist() {
  try {
    const r = await fetch(`${BASE}/api/whitelist`);
    const d = await r.json();
    const entries = d.entries || [];
    const badge = document.getElementById('wl-badge');
    badge.textContent = `${entries.length} key${entries.length !== 1 ? 's' : ''}`;

    const empty = document.getElementById('wl-empty');
    const table = document.getElementById('wl-table');
    const body  = document.getElementById('wl-body');

    if (entries.length === 0) {
      empty.style.display = '';
      table.style.display = 'none';
      return;
    }
    empty.style.display = 'none';
    table.style.display = '';
    body.innerHTML = '';
    for (const e of entries) {
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>
          <span class="npub" title="${e.npub}">${npubShort(e.npub)}</span>
          <div style="font-size:0.72rem;color:var(--muted);margin-top:2px;font-family:monospace">
            ${e.pubkey.slice(0, 16)}…
          </div>
        </td>
        <td class="note-text">${e.note || '—'}</td>
        <td class="date-text">${formatDate(e.added_at)}</td>
        <td style="text-align:right">
          <button class="btn btn-danger" onclick="removeKey('${e.pubkey}', this)">Remove</button>
        </td>
      `;
      body.appendChild(tr);
    }
  } catch(e) { console.error('whitelist error', e); }
}

async function addKey(evt) {
  evt.preventDefault();
  const val = document.getElementById('inp-npub').value.trim();
  const note = document.getElementById('inp-note').value.trim();
  if (!val) return;

  const body = val.startsWith('npub') ? { npub: val, note: note || null }
                                       : { pubkey: val, note: note || null };
  try {
    const r = await fetch(`${BASE}/api/whitelist`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const d = await r.json();
    if (r.ok) {
      toast('Key added to whitelist');
      document.getElementById('inp-npub').value = '';
      document.getElementById('inp-note').value = '';
      loadWhitelist();
      loadStats();
    } else {
      toast(d.error || 'Error', 'error');
    }
  } catch(e) { toast('Network error', 'error'); }
}

async function removeKey(pubkey, btn) {
  btn.disabled = true;
  try {
    const r = await fetch(`${BASE}/api/whitelist/${pubkey}`, { method: 'DELETE' });
    if (r.ok) {
      toast('Key removed');
      loadWhitelist();
      loadStats();
    } else {
      const d = await r.json();
      toast(d.error || 'Error', 'error');
      btn.disabled = false;
    }
  } catch(e) { toast('Network error', 'error'); btn.disabled = false; }
}

// init
document.getElementById('ws-url').textContent = relayWsUrl();
loadStats();
loadWhitelist();
setInterval(loadStats, 10000);
</script>
</body>
</html>
"#;
