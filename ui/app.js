/* ══════════════════════════════════════════════════════════════════════════
   Foldu — lớp giao diện
   ══════════════════════════════════════════════════════════════════════════ */
'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $  = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

// ─────────────────────────────────────────────────────────────── Trạng thái

const S = {
  sources: [],
  profile: null,
  settings: null,
  catalog: [],
  presets: [],
  analytics: null,
  plan: null,
  deselected: new Set(),
  skipIds: [],
  filter: '',
  search: '',
  listTotal: 0,
  pages: new Map(),      // số trang -> mảng thao tác
  pending: new Set(),
  activePreset: null,
  recoPreset: null,       // kiểu được gợi ý theo kết quả phân tích
  showAllPresets: false,
  listOpen: false,        // danh sách chi tiết ở bước 4 mặc định gấp lại
  lastSession: null,
};

const PAGE = 200;
const ROW_H = 46;

// ───────────────────────────────────────────────────────────────── Tiện ích

function fmtBytes(n) {
  n = Number(n) || 0;
  if (n < 1024) return n + ' B';
  const u = ['KB', 'MB', 'GB', 'TB', 'PB'];
  let x = n / 1024, i = 0;
  while (x >= 1024 && i < u.length - 1) { x /= 1024; i++; }
  return (x >= 100 ? x.toFixed(0) : x >= 10 ? x.toFixed(1) : x.toFixed(2)) + ' ' + u[i];
}

function fmtNum(n) { return (Number(n) || 0).toLocaleString(currentLang() === 'en' ? 'en-GB' : 'vi-VN'); }

function fmtDate(ms) {
  if (!ms) return '—';
  return new Intl.DateTimeFormat(currentLang() === 'en' ? 'en-GB' : 'vi-VN', {
    day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit',
  }).format(new Date(Number(ms)));
}

function fmtDur(ms) {
  const en = currentLang() === 'en';
  const s = Math.round(ms / 1000);
  if (s < 60) return s + (en ? ' sec' : ' giây');
  const m = Math.floor(s / 60);
  if (m < 60) return en ? `${m} min ${s % 60} sec` : `${m} phút ${s % 60} giây`;
  return en ? `${Math.floor(m / 60)} h ${m % 60} min` : `${Math.floor(m / 60)} giờ ${m % 60} phút`;
}

function esc(s) {
  return String(s == null ? '' : s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function baseName(p) {
  const s = String(p || '');
  const i = Math.max(s.lastIndexOf('\\'), s.lastIndexOf('/'));
  return i >= 0 ? s.slice(i + 1) : s;
}

/** Rút gọn đường dẫn giữ phần đuôi (phần có ý nghĩa nhất) */
function tailPath(p, keep = 3) {
  const parts = String(p || '').split(/[\\/]/).filter(Boolean);
  return parts.length <= keep ? parts.join('\\') : '…\\' + parts.slice(-keep).join('\\');
}

function toast(msg, kind = 'ok', ms = 3600) {
  const icons = { ok: '#i-check', err: '#i-x', warn: '#i-warn' };
  const el = document.createElement('div');
  el.className = 'toast ' + kind;
  el.innerHTML = `<svg><use href="${icons[kind] || icons.ok}"/></svg><span>${esc(msg)}</span>`;
  $('#toasts').appendChild(el);
  setTimeout(() => { el.style.opacity = '0'; setTimeout(() => el.remove(), 220); }, ms);
}

function debounce(fn, ms) {
  let t;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    const msg = typeof e === 'string' ? e : (e && e.message) || String(e);
    toast(msg, 'err', 6000);
    throw e;
  }
}

// ────────────────────────────────────────────────────────────── Điều hướng

const STEPS = ['home', 'analyze', 'design', 'preview'];

function go(screen) {
  $$('.screen').forEach((s) => s.classList.toggle('active', s.id === 's-' + screen));

  const at = STEPS.indexOf(screen);
  $$('.nav-item').forEach((b) => {
    const i = STEPS.indexOf(b.dataset.go);
    b.classList.toggle('active', b.dataset.go === screen);
    // Bước đã đi qua hiện dấu tích, để người dùng biết mình đang ở đâu trong quy trình
    b.classList.toggle('done', i >= 0 && at >= 0 && i < at && !b.disabled);
  });

  if (screen === 'history') loadHistory();
  if (screen === 'settings') renderSettings();
  if (screen === 'preview') {
    // Danh sách ảo tính số dòng theo chiều cao khung nhìn. Khi khối còn gấp lại thì
    // chiều cao bằng 0, nên chỉ vẽ khi nó đang mở. Gọi đồng bộ để việc đọc
    // clientHeight buộc trình duyệt tính lại layout ngay, không đợi khung sau.
    if (S.listOpen) drawRows();
    // Kiểm tra an toàn phải chạy dù người dùng vào bước này bằng đường nào,
    // vì nút Bắt đầu dọn chỉ mở khoá sau khi kiểm tra xong.
    runPreflight();
  }
}

function unlock(...names) {
  names.forEach((n) => {
    const b = $(`.nav-item[data-go="${n}"]`);
    if (b) b.disabled = false;
  });
}

document.addEventListener('click', (e) => {
  const b = e.target.closest('[data-go]');
  if (b && !b.disabled) go(b.dataset.go);
});

// ────────────────────────────────────────────────────────────── Giao diện sáng/tối

$('#theme-toggle').onclick = () => {
  const cur = document.documentElement.dataset.theme;
  const next = cur === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = next;
  if (S.settings) { S.settings.theme = next; invoke('set_settings', { s: S.settings }).catch(() => {}); }
};

// ────────────────────────────────────────────────────────────── Đổi ngôn ngữ

/**
 * Đổi ngôn ngữ cho CẢ hai lớp. Lõi Rust cũng phải đổi vì tên thư mục nó sinh ra
 * (01-Hinh-Anh so với 01-Images), danh mục cách chia và mẫu dựng sẵn đều nằm bên đó.
 * Sau đó vẽ lại mọi thứ đang hiện, kể cả kế hoạch đã tính.
 */
async function applyLang(code) {
  setLang(code);
  applyI18n();
  $('#flag-now').innerHTML = `<use href="#flag-${code}"/>`;
  $('#lang-code').textContent = code.toUpperCase();

  const st = await invoke('set_lang', { lang: code }).catch(() => null);
  if (st) S.settings = st;
  const [presets, catalog] = await Promise.all([
    invoke('get_presets').catch(() => S.presets),
    invoke('get_catalog').catch(() => S.catalog),
  ]);
  S.presets = presets;
  S.catalog = catalog;

  renderPresets();
  renderLayers();
  syncControls();
  renderRecent();
  renderDrives();
  readScope();
  syncAnFold();
  syncListToggle();
  if (S.analytics) renderAnalytics();
  if (S.plan) refreshPlan();

  const now = document.querySelector('.screen.active')?.id.replace('s-', '');
  if (now === 'history') loadHistory();
  if (now === 'settings') renderSettings();
}

$('#lang-toggle').onclick = () => applyLang(currentLang() === 'vi' ? 'en' : 'vi');

/**
 * Lần mở đầu tiên thì hỏi ngôn ngữ. Hộp này cố tình viết song ngữ vì lúc đó
 * chưa biết người dùng đọc được thứ tiếng nào, và không đóng được bằng cách bấm
 * ra ngoài hay bấm Esc: phải chọn một cái.
 */
function showLangPicker() {
  const m = $('#lang-modal');
  const guess = currentLang();
  const hint = { vi: 'Ngôn ngữ máy bạn đang dùng', en: 'Your system language' };

  $$('[data-pick-lang]').forEach((b) => {
    const code = b.dataset.pickLang;
    b.classList.toggle('suggested', code === guess);
    if (code === guess) b.dataset.hint = hint[code];
    b.onclick = async () => {
      m.classList.add('hidden');
      if (code === currentLang()) {
        // Giữ nguyên ngôn ngữ đã đoán, chỉ cần ghi nhận là đã hỏi rồi
        const s = await invoke('confirm_lang').catch(() => null);
        if (s) S.settings = s;
      } else {
        await applyLang(code);
      }
      checkInterrupted();
    };
  });
  m.classList.remove('hidden');
  setTimeout(() => $(`[data-pick-lang="${guess}"]`)?.focus(), 60);
}

// ═══════════════════════════════════════════════════════════ 1. Màn Bắt đầu

async function addSources(paths) {
  for (const p of paths) {
    if (S.sources.some((s) => s.path.toLowerCase() === p.toLowerCase())) continue;
    const chk = await invoke('check_path', { path: p }).catch(() => ({ ok: false, level: 'block', reason: '?' }));
    S.sources.push({ path: p, check: chk });
  }
  renderSources();
}

function renderSources() {
  const box = $('#src-list');
  box.innerHTML = S.sources.map((s, i) => `
    <div class="src-item">
      <svg><use href="#i-folder"/></svg>
      <span class="p" title="${esc(s.path)}">${esc(s.path)}</span>
      ${s.check.level === 'warn' ? `<span class="warn">${esc(s.check.reason)}</span>` : ''}
      ${s.check.level === 'block' ? `<span class="bad">${esc(s.check.reason)}</span>` : ''}
      <button class="icon-btn" data-rm="${i}"><svg><use href="#i-x"/></svg></button>
    </div>`).join('');
  $$('[data-rm]', box).forEach((b) => {
    b.onclick = () => { S.sources.splice(+b.dataset.rm, 1); renderSources(); renderDrives(); };
  });
  $('#home-actions').classList.toggle('hidden', S.sources.length === 0);
  $('#btn-scan').disabled = !S.sources.some((s) => s.check.ok);
}

$('#btn-pick').onclick = async () => {
  const paths = await invoke('pick_folders');
  if (paths && paths.length) addSources(paths);
};

// Kéo thả từ Explorer
listen('tauri://drag-enter', () => $('#drop').classList.add('over'));
listen('tauri://drag-leave', () => $('#drop').classList.remove('over'));
listen('tauri://drag-drop', (e) => {
  $('#drop').classList.remove('over');
  const paths = (e.payload && e.payload.paths) || [];
  if (paths.length) { go('home'); addSources(paths); }
});

/**
 * Phạm vi dọn là quyết định có hậu quả lớn nhất ở màn đầu, nên nó được đọc lại
 * mỗi lần đổi và tóm tắt bằng một câu ngay cạnh nút Bước tiếp theo.
 */
function readScope() {
  const deep = $('input[name="scope"]:checked')?.value !== 'flat';
  S.profile.recursive = deep;
  // Chỉ dọn vỏ rỗng khi thật sự có lôi file ra khỏi thư mục con
  S.profile.safety.cleanEmptyDirs = deep && $('#opt-clean-empty').checked;

  $('.scope-sub').style.display = deep ? '' : 'none';
  $('#scope-recap').textContent = t(deep
    ? (S.profile.safety.cleanEmptyDirs ? 'scope.recap.deepClean' : 'scope.recap.deep')
    : 'scope.recap.flat');
}

$$('input[name="scope"]').forEach((r) => { r.onchange = readScope; });
$('#opt-clean-empty').onchange = readScope;

/**
 * Quét thư mục. `recommend` chỉ bật ở lần quét đầu: lúc đó chọn sẵn kiểu sắp xếp
 * hợp lý rồi đưa người dùng qua màn Phân tích. Khi quét lại từ banner thì giữ
 * nguyên mọi thiết lập họ đã chỉnh và trả họ về đúng màn đang đứng.
 */
async function runScan(recommend) {
  const roots = S.sources.filter((s) => s.check.ok).map((s) => s.path);
  if (!roots.length) return;
  const back = document.querySelector('.screen.active')?.id.replace('s-', '') || 'analyze';
  readScope();

  showRun(t('run.scanning'), false);
  const res = await call('scan_folders', { paths: roots, profile: S.profile });

  S.analytics = res.analytics;
  S.skipped = res.skipped;
  if (res.blocked && res.blocked.length) toast(res.blocked.join(' · '), 'warn', 6000);

  rememberRecent(roots);
  unlock('analyze', 'design', 'preview');
  renderAnalytics();
  syncAnFold();
  clearStale();
  if (recommend) applyRecommendedPreset();   // chọn sẵn kiểu hợp với thư mục vừa quét
  go(recommend ? 'analyze' : back);
  refreshPlan();
}

$('#btn-scan').onclick = () => runScan(true);

/**
 * Vài lựa chọn an toàn đổi cái phần mềm được phép nhìn thấy, nên phải quét lại mới
 * có tác dụng. Trước đây chỗ này chỉ hiện một toast rồi biến mất trong ba giây:
 * người dùng tưởng thay đổi đã áp, và không có đường nào quét lại ngay tại chỗ.
 */
function markPlanStale() {
  $('#stale-design').classList.remove('hidden');
  $('#stale-preview').classList.remove('hidden');
}
function clearStale() {
  $('#stale-design').classList.add('hidden');
  $('#stale-preview').classList.add('hidden');
}
$$('[data-stale-go]').forEach((b) => { b.onclick = () => runScan(false); });

function rememberRecent(roots) {
  if (!S.settings) return;
  const list = S.settings.recentPaths || [];
  for (const r of roots) {
    const i = list.findIndex((x) => x.toLowerCase() === r.toLowerCase());
    if (i >= 0) list.splice(i, 1);
    list.unshift(r);
  }
  S.settings.recentPaths = list.slice(0, 8);
  invoke('set_settings', { s: S.settings }).catch(() => {});
  renderRecent();
}

// ── Chọn cả ổ đĩa / phân vùng
async function renderDrives() {
  const drives = await invoke('list_drives').catch(() => []);
  S.drives = drives;
  const picked = new Set(S.sources.map((s) => s.path.toLowerCase()));

  $('#drives').innerHTML = drives.map((d) => {
    const usedPct = d.total ? (d.used / d.total) * 100 : 0;
    const tight = d.total && d.free / d.total < 0.12;
    const isPicked = picked.has(d.path.toLowerCase());
    return `
      <button class="drive ${tight ? 'tight' : ''} ${isPicked ? 'picked' : ''}"
              data-drive="${esc(d.path)}" ${d.selectable ? '' : 'disabled'}
              title="${esc(d.note || d.path)}">
        <div class="drive-top">
          <span class="drive-letter">${esc(d.letter)}:</span>
          <span class="drive-label">${esc(d.label)}</span>
          ${d.isSystem ? `<span class="drive-lock">${t('drive.system')}</span>` : ''}
        </div>
        <div class="drive-bar"><i style="width:${usedPct.toFixed(1)}%"></i></div>
        <div class="drive-meta">${fmtBytes(d.free)} trống / ${fmtBytes(d.total)}</div>
        ${d.note ? `<div class="drive-note">${esc(d.note)}</div>` : ''}
      </button>`;
  }).join('') || `<p class="muted sm">${t('chart.none')}</p>`;

  $$('[data-drive]').forEach((b) => {
    b.onclick = () => {
      const p = b.dataset.drive;
      const i = S.sources.findIndex((s) => s.path.toLowerCase() === p.toLowerCase());
      if (i >= 0) { S.sources.splice(i, 1); renderSources(); renderDrives(); }
      else addSources([p]).then(renderDrives);
    };
  });
}

function renderRecent() {
  const list = (S.settings && S.settings.recentPaths) || [];
  const box = $('#recent');
  if (!list.length) { box.innerHTML = ''; return; }
  box.innerHTML = `<h2>${t('home.recent')}</h2>` + list.map((p) => `<button data-recent="${esc(p)}">${esc(p)}</button>`).join('');
  $$('[data-recent]', box).forEach((b) => { b.onclick = () => addSources([b.dataset.recent]); });
}

// ═══════════════════════════════════════════════════════════ 2. Màn Phân tích

function renderAnalytics() {
  const a = S.analytics;
  if (!a) return;

  $('#an-sub').textContent = t('an.sub', fmtNum(a.totalFiles), fmtBytes(a.totalBytes), fmtNum(a.scannedDirs));

  // ── Thanh dung lượng ổ (chỉ khi quét nguyên phân vùng)
  const card = $('#drive-card');
  card.classList.toggle('hidden', !a.drive);
  if (a.drive) {
    const d = a.drive;
    $('#drive-title').textContent = `Ổ ${d.letter}: ${d.label}`;
    $('#drive-fs').textContent = d.fileSystem || '';
    const pct = (v) => (d.total ? (v / d.total) * 100 : 0);
    $('#cap-scope').style.width = pct(a.totalBytes).toFixed(2) + '%';
    $('#cap-out').style.width = pct(a.outOfScopeBytes).toFixed(2) + '%';
    $('#cap-legend').innerHTML = `
      <span><i style="background:var(--accent)"></i>${t('cap.inScope')} <b>${fmtBytes(a.totalBytes)}</b></span>
      <span><i style="background:var(--border-2)"></i>${t('cap.protected')} <b>${fmtBytes(a.outOfScopeBytes)}</b></span>
      <span><i style="background:var(--surface-2)"></i>${t('cap.free')} <b>${fmtBytes(d.free)}</b></span>`;
  }

  // Bốn ô đầu là thứ ai cũng cần biết; phần còn lại thuộc khối chi tiết gấp lại,
  // để màn này mở ra chỉ có một câu kết luận và vài con số, không phải bảng dày đặc.
  const main = [
    { b: fmtNum(a.totalFiles), s: t('stat.files') },
    { b: fmtBytes(a.totalBytes), s: t('stat.bytes') },
    { b: fmtNum(a.filesAtRoot), s: t('stat.atRoot'), k: a.filesAtRoot > a.totalFiles * 0.25 ? 'warn' : '' },
    { b: fmtBytes(a.coldBytes), s: t('stat.cold', fmtNum(a.coldCount)), k: a.coldCount ? 'warn' : '' },
  ];
  const extra = [
    { b: fmtNum(a.unnamedCount), s: t('stat.unnamed'), k: a.unnamedCount ? 'warn' : '' },
    { b: a.maxDepth, s: t('stat.depth') },
  ];
  if (a.projectFolders) extra.push({ b: fmtNum(a.projectFolders), s: t('stat.projects'), k: 'good' });
  if (a.appFoldersProtected) extra.push({ b: fmtNum(a.appFoldersProtected), s: t('stat.apps'), k: 'good' });
  if (a.systemProtected) extra.push({ b: fmtNum(a.systemProtected), s: t('stat.system'), k: 'good' });
  if (a.cloudSkipped) extra.push({ b: fmtNum(a.cloudSkipped), s: t('stat.cloud'), k: 'warn' });

  const statHtml = (list) => list.map((x) =>
    `<div class="stat ${x.k || ''}"><b>${x.b}</b><span>${esc(x.s)}</span></div>`).join('');
  $('#an-stats').innerHTML = statHtml(main);
  $('#an-stats-extra').innerHTML = statHtml(extra);

  // Kết luận bằng một câu, đặt to ngay đầu màn — người mới đọc câu này là đủ hiểu
  const ring = $('#health-ring');
  ring.style.setProperty('--v', a.health);
  ring.style.setProperty('--ring-color',
    a.health >= 70 ? 'var(--green)' : a.health >= 40 ? 'var(--amber)' : 'var(--red)');
  $('#health-num').textContent = a.health;
  $('#verdict-title').textContent = t(
    a.health >= 80 ? 'verdict.great' : a.health >= 60 ? 'verdict.ok'
      : a.health >= 40 ? 'verdict.messy' : 'verdict.bad');
  $('#health-notes').innerHTML = a.healthNotes.map((n) => `<li>${esc(n)}</li>`).join('');

  // Dung lượng theo loại
  const maxT = Math.max(1, ...a.byType.map((b) => b.bytes));
  $('#chart-type').innerHTML = a.byType.map((b) => `
    <div class="bar-row">
      <span class="n" title="${esc(b.label)}">${esc(b.label)}</span>
      <div class="bar-track"><div class="bar-fill" style="width:${(b.bytes / maxT * 100).toFixed(1)}%"></div></div>
      <span class="v">${fmtBytes(b.bytes)}</span>
    </div>`).join('') || `<p class="muted sm">${t('chart.none')}</p>`;

  // Thư mục cấp 1 chiếm nhiều nhất
  const maxF = Math.max(1, ...a.byFolder.map((b) => b.bytes));
  $('#chart-folder').innerHTML = a.byFolder.map((b) => `
    <div class="bar-row">
      <span class="n" title="${esc(b.label)}">${esc(b.label)}</span>
      <div class="bar-track"><div class="bar-fill" style="width:${(b.bytes / maxF * 100).toFixed(1)}%"></div></div>
      <span class="v">${fmtBytes(b.bytes)}</span>
    </div>`).join('') || `<p class="muted sm">${t('chart.none')}</p>`;

  // Số file theo năm
  const years = a.byYear.slice(-14);
  const maxY = Math.max(1, ...years.map((b) => b.count));
  $('#chart-year').innerHTML = years.map((b) => `
    <div class="col" title="${esc(b.label)}: ${fmtNum(b.count)} file · ${fmtBytes(b.bytes)}">
      <div class="b" style="height:${(b.count / maxY * 100).toFixed(1)}%"></div>
      <span class="l">${esc(b.label.slice(-2))}</span>
    </div>`).join('') || `<p class="muted sm">${t('chart.none')}</p>`;

  // 20 file lớn nhất
  $('#top-files').innerHTML = a.topFiles.map((f) => `
    <div class="tlist-row" data-reveal="${esc(f.path)}">
      <span class="n" title="${esc(f.path)}">${esc(f.name)}</span>
      <span class="s">${fmtBytes(f.size)}</span>
    </div>`).join('') || `<p class="muted sm pad16">${t('chart.none')}</p>`;

  // File đã bỏ qua
  const sk = S.skipped || [];
  $('#skipped-card').classList.toggle('hidden', sk.length === 0);
  $('#skip-count').textContent = fmtNum(sk.length);
  $('#skipped-list').innerHTML = sk.slice(0, 200).map((s) => `
    <div class="tlist-row" data-reveal="${esc(s.path)}">
      <span class="n" title="${esc(s.path)}">${esc(baseName(s.path))}</span>
      <span class="s">${esc(s.reason)}</span>
    </div>`).join('');

  $$('[data-reveal]').forEach((el) => {
    el.onclick = () => invoke('reveal', { path: el.dataset.reveal }).catch(() => {});
  });
}

/** Gấp / mở khối biểu đồ chi tiết ở màn Phân tích */
function syncAnFold() {
  const open = $('#an-more').getAttribute('aria-expanded') === 'true';
  $('#an-details').classList.toggle('hidden', !open);
  $('#an-more span').textContent = t(open ? 'an.less' : 'an.more');
}
$('#an-more').onclick = () => {
  const b = $('#an-more');
  b.setAttribute('aria-expanded', String(b.getAttribute('aria-expanded') !== 'true'));
  syncAnFold();
};

// ═══════════════════════════════════════════════════════════ 3. Màn Thiết kế

const MODE_KEY = { Move: 'mode.move', Copy: 'mode.copy', Hardlink: 'mode.link', ReportOnly: 'mode.report' };

/** Ví dụ đường dẫn kết quả — người dùng thấy ngay kiểu này ra cái gì */
const PRESET_EG_IDS = ['downloads', 'photos', 'projects', 'company', 'diskfull', 'media', 'archive'];
const presetEg = (id) => (PRESET_EG_IDS.includes(id) ? t('eg.' + id) : null);

/**
 * Đoán kiểu sắp xếp hợp nhất với thư mục vừa quét.
 *
 * Đây là quyết định trung tâm của cả phần mềm. Bày tám thẻ ngang hàng, không thẻ
 * nào được chọn sẵn, là đẩy trọn gánh nặng sang người dùng — đúng chỗ người mới
 * bị tê liệt. Chỉ dựa vào dấu hiệu cấu trúc đọc được từ bản phân tích, không đoán
 * theo chữ trên nhãn vì nhãn đổi theo ngôn ngữ.
 */
function recommendPreset() {
  const a = S.analytics;
  if (!a) return null;
  const has = (id) => S.presets.some((p) => p.id === id);
  if (a.drive && has('diskfull')) return 'diskfull';              // quét nguyên ổ: lo dung lượng trước
  if (a.projectFolders >= 3 && has('projects')) return 'projects';
  if (a.totalBytes && a.coldBytes / a.totalBytes > 0.4 && has('archive')) return 'archive';
  return has('downloads') ? 'downloads' : (S.presets[0] && S.presets[0].id) || null;
}

/** Áp một kiểu dựng sẵn vào hồ sơ đang làm */
function usePreset(p, replan) {
  S.activePreset = p.id;
  S.profile.layers = p.layers.slice();
  S.profile.mode = p.mode;
  if (p.id === 'dupes') { S.profile.duplicates.enabled = true; S.profile.duplicates.action = 'Report'; }
  syncControls();
  renderPresets();
  renderLayers();
  if (replan) refreshPlan();
}

/** Sau khi quét xong thì chọn sẵn kiểu hợp lý, người dùng chỉ cần bấm Bước tiếp theo */
function applyRecommendedPreset() {
  S.recoPreset = recommendPreset();
  const p = S.presets.find((x) => x.id === S.recoPreset);
  $('#reco-note').classList.toggle('hidden', !p);
  if (p) usePreset(p, false);
  else renderPresets();
}

const PRESET_HEAD = 3;   // số thẻ hiện lúc đầu, phần còn lại nằm sau nút Xem thêm

function renderPresets() {
  // Kiểu được gợi ý luôn đứng đầu, phần còn lại giữ nguyên thứ tự của lõi
  const ordered = S.recoPreset
    ? [...S.presets.filter((p) => p.id === S.recoPreset), ...S.presets.filter((p) => p.id !== S.recoPreset)]
    : S.presets.slice();
  const showAll = S.showAllPresets || !S.recoPreset;
  const shown = showAll ? ordered : ordered.slice(0, PRESET_HEAD);

  $('#presets').innerHTML = shown.map((p) => `
    <button class="preset ${S.activePreset === p.id ? 'active' : ''}" data-preset="${p.id}">
      ${p.id === S.recoPreset ? `<span class="preset-badge">${t('de.reco.badge')}</span>` : ''}
      <b>${esc(p.name)}</b><span>${esc(p.desc)}</span>
      ${presetEg(p.id) ? `<span class="eg">${esc(presetEg(p.id))}</span>` : ''}
    </button>`).join('');

  const rest = ordered.length - PRESET_HEAD;
  const more = $('#btn-more-presets');
  more.classList.toggle('hidden', !S.recoPreset || rest <= 0);
  more.textContent = showAll ? t('de.showLess') : t('de.showMore', fmtNum(rest));
  more.setAttribute('aria-expanded', String(!!showAll));

  $$('[data-preset]').forEach((b) => {
    b.onclick = () => usePreset(S.presets.find((x) => x.id === b.dataset.preset), true);
  });
}

$('#btn-more-presets').onclick = () => { S.showAllPresets = !S.showAllPresets; renderPresets(); };

function critById(id) {
  for (const g of S.catalog) {
    const it = g.items.find((x) => x.id === id);
    if (it) return it;
  }
  return null;
}

/**
 * Người dùng phổ thông không đọc được "%Y/%m". Cho chọn bằng lời, kèm ví dụ thật.
 * Chia hai nhóm vì một mức có thể là một mốc đơn (Năm), hoặc tự tách thành
 * nhiều thư mục lồng nhau (Năm rồi tháng).
 */
const TIME_FORMATS = [
  { g: 'fmt.group.one', v: '%Y' },
  { g: 'fmt.group.one', v: '%m' },
  { g: 'fmt.group.one', v: '%B' },
  { g: 'fmt.group.one', v: '%q' },
  { g: 'fmt.group.one', v: '%W' },
  { g: 'fmt.group.one', v: '%d' },
  { g: 'fmt.group.split', v: '%Y/%m' },
  { g: 'fmt.group.split', v: '%Y/%B' },
  { g: 'fmt.group.split', v: '%Y/%m/%d' },
  { g: 'fmt.group.join', v: '%Y-%q' },
  { g: 'fmt.group.join', v: '%Y-%W' },
  { g: 'fmt.group.join', v: '%Y-%m' },
];

function timeFormatOptions(cur) {
  let html = '';
  let group = null;
  for (const f of TIME_FORMATS) {
    if (f.g !== group) {
      if (group !== null) html += '</optgroup>';
      html += `<optgroup label="${esc(t(f.g))}">`;
      group = f.g;
    }
    html += `<option value="${esc(f.v)}"${f.v === cur ? ' selected' : ''}>${esc(t('fmt.' + f.v))}</option>`;
  }
  if (group !== null) html += '</optgroup>';
  if (!TIME_FORMATS.some((f) => f.v === cur)) {
    html += `<option value="${esc(cur)}" selected>${esc(t('fmt.custom'))} · ${esc(cur)}</option>`;
  }
  return html;
}

function renderLayers() {
  const box = $('#layers');
  const layers = S.profile.layers;
  box.innerHTML = layers.map((spec, i) => {
    const [id, arg] = splitSpec(spec);
    const c = critById(id);
    let control = '';
    if (c && c.arg !== undefined) {
      const cur = arg || c.arg;
      if (String(c.arg).includes('%')) {
        control = `<select data-arg="${i}">${timeFormatOptions(cur)}</select>`;
      } else {
        control = `<input type="text" data-arg="${i}" value="${esc(cur)}" placeholder="${esc(c.arg)}">`;
      }
    }
    return `
      <div class="layer">
        <span class="idx">${i + 1}</span>
        <div class="body">
          <b class="pick" data-pick="${i}" title="${esc(t('de.pickCrit'))}">${esc(c ? c.name : id)}</b>
          ${control}
        </div>
        <button class="del" data-del="${i}" title="${esc(t('de.removeLevel'))}"><svg><use href="#i-x"/></svg></button>
      </div>`;
  }).join('') || `<p class="muted sm">${t('de.noLevels')}</p>`;

  $$('[data-pick]', box).forEach((b) => { b.onclick = () => openCritModal(+b.dataset.pick); });
  $$('[data-del]', box).forEach((b) => {
    b.onclick = () => { layers.splice(+b.dataset.del, 1); S.activePreset = null; renderPresets(); renderLayers(); refreshPlan(); };
  });

  const setArg = (i, value) => {
    const [id] = splitSpec(layers[i]);
    layers[i] = value ? `${id}:${value}` : id;
    S.activePreset = null;
    renderPresets();
    refreshPlan();
  };
  $$('select[data-arg]', box).forEach((sel) => { sel.onchange = () => setArg(+sel.dataset.arg, sel.value); });
  $$('input[data-arg]', box).forEach((inp) => {
    inp.oninput = debounce(() => setArg(+inp.dataset.arg, inp.value), 500);
  });

  $('#btn-add-layer').disabled = layers.length >= 4;
  $('#cluster-block').classList.toggle('hidden', !layers.some((l) => splitSpec(l)[0] === 'AUTO_PROJECT'));
  updateAccSummaries();
}

function splitSpec(spec) {
  const i = String(spec).indexOf(':');
  return i < 0 ? [spec, ''] : [spec.slice(0, i), spec.slice(i + 1)];
}

// ── Hộp chọn tiêu chí
let critTarget = -1;

function openCritModal(index) {
  critTarget = index;
  $('#crit-search').value = '';
  renderCritList('');
  $('#crit-modal').classList.remove('hidden');
  setTimeout(() => $('#crit-search').focus(), 40);
}

function renderCritList(q) {
  const needle = q.trim().toLowerCase();
  let html = '';
  for (const g of S.catalog) {
    const items = g.items.filter((it) =>
      !needle || it.name.toLowerCase().includes(needle) || it.id.toLowerCase().includes(needle)
      || it.desc.toLowerCase().includes(needle));
    if (!items.length) continue;
    html += `<div class="crit-group">${esc(g.group)}</div>`;
    html += items.map((it) => `
      <button class="crit" data-crit="${esc(it.id)}" data-defarg="${esc(it.arg || '')}">
        <b>${esc(it.name)}${it.slow ? `<span class="slow">${t('crit.slow')}</span>` : ''}</b>
        <span>${esc(it.desc)}</span>
      </button>`).join('');
  }
  $('#crit-list').innerHTML = html || `<p class="muted sm" style="padding:20px">${t('crit.none')}</p>`;
  $$('[data-crit]').forEach((b) => {
    b.onclick = () => {
      const id = b.dataset.crit;
      const arg = b.dataset.defarg;
      const spec = arg ? `${id}:${arg}` : id;
      if (critTarget < 0) S.profile.layers.push(spec);
      else S.profile.layers[critTarget] = spec;
      S.activePreset = null;
      $('#crit-modal').classList.add('hidden');
      renderPresets(); renderLayers(); refreshPlan();
    };
  });
}

$('#crit-search').oninput = (e) => renderCritList(e.target.value);
$('#crit-close').onclick = () => $('#crit-modal').classList.add('hidden');
$('#crit-modal').onclick = (e) => { if (e.target.id === 'crit-modal') $('#crit-modal').classList.add('hidden'); };
$('#btn-add-layer').onclick = () => openCritModal(-1);

// ── Các điều khiển còn lại
function syncControls() {
  const p = S.profile;
  $$('#mode-seg button').forEach((b) => {
    const on = b.dataset.mode === p.mode;
    b.classList.toggle('active', on);
    b.setAttribute('aria-pressed', String(on));   // máy đọc màn hình cần biết nút nào đang chọn
  });
  // Chế độ Chỉ xem thử không dọn gì cả, nên nút cuối cùng phải nói đúng việc nó làm
  $('#btn-apply').textContent = t(p.mode === 'ReportOnly' ? 'pv.apply.report' : 'pv.apply');
  $('#mode-hint').textContent = MODE_KEY[p.mode] ? t(MODE_KEY[p.mode]) : '';
  $('#granularity').value = p.clustering.granularity;
  $('#dup-enabled').checked = p.duplicates.enabled;
  $('#dup-strategy').value = p.duplicates.strategy;
  $('#dup-action').value = p.duplicates.action;
  $('#near-enabled').checked = p.duplicates.nearImages !== false;
  $('#near-threshold').value = String(p.duplicates.nearThreshold || 10);
  $('#near-opts').classList.toggle('hidden', p.duplicates.nearImages === false);
  $('#dup-opts').classList.toggle('hidden', !p.duplicates.enabled);
  $('#sf-sidecar').checked = p.safety.keepSidecarTogether;
  $('#sf-project').checked = p.safety.treatProjectFoldersAsUnit;
  $('#sf-apps').checked = p.safety.protectInstalledApps !== false;
  $('#sf-cloud').checked = p.filters.skipCloudPlaceholder;
  $('#sf-prefix').checked = p.numberPrefix;
  $('#dest-path').value = p.destination || '';

  // Đổi tên hàng loạt
  const rn = p.rename || (p.rename = { enabled: false, inPlace: true, parts: [], transforms: {} });
  $('#rename-enabled').checked = !!rn.enabled;
  $('#rename-panel').classList.toggle('hidden', !rn.enabled);
  $('#rn-strip').checked = !!(rn.transforms && rn.transforms.stripDiacritics);
  $('#rn-lower').checked = !!(rn.transforms && rn.transforms.lowercase);
  $('#rn-kebab').checked = !!(rn.transforms && rn.transforms.kebab);
  $$('#rn-place button').forEach((b) => {
    const on = (b.dataset.place === 'in') === (rn.inPlace !== false);
    b.classList.toggle('active', on);
    b.setAttribute('aria-pressed', String(on));
  });
  renderRenameChips();
  updateAccSummaries();
}

$$('#mode-seg button').forEach((b) => {
  b.onclick = () => { S.profile.mode = b.dataset.mode; syncControls(); refreshPlan(); };
});
$('#granularity').oninput = debounce((e) => {
  S.profile.clustering.granularity = +e.target.value; refreshPlan();
}, 320);
$('#dup-enabled').onchange = (e) => { S.profile.duplicates.enabled = e.target.checked; syncControls(); refreshPlan(); };
$('#dup-strategy').onchange = (e) => { S.profile.duplicates.strategy = e.target.value; refreshPlan(); };
$('#dup-action').onchange = (e) => { S.profile.duplicates.action = e.target.value; refreshPlan(); };
$('#near-enabled').onchange = (e) => { S.profile.duplicates.nearImages = e.target.checked; syncControls(); refreshPlan(); };
$('#near-threshold').onchange = (e) => { S.profile.duplicates.nearThreshold = +e.target.value; refreshPlan(); };

// ─────────────────────────────────────────── Đổi tên hàng loạt: bộ lắp mẫu

const DATE_FMTS = [
  { v: '%Y-%m-%d', k: 'rn.fmt.ymd', s: '2026-03-15' },
  { v: '%Y-%m', k: 'rn.fmt.ym', s: '2026-03' },
  { v: '%Y', k: 'rn.fmt.y', s: '2026' },
  { v: '%d-%m-%Y', k: 'rn.fmt.dmy', s: '15-03-2026' },
];
const rnParts = () => (S.profile.rename && S.profile.rename.parts) || [];

/** Vẽ chuỗi mảnh ghép, mỗi mảnh có điều khiển riêng và nút gỡ */
function renderRenameChips() {
  const box = $('#rn-chips');
  const parts = rnParts();
  $('#rn-empty').classList.toggle('hidden', parts.length > 0);

  box.innerHTML = parts.map((p, i) => {
    let inner = '';
    if (p.kind === 'text') {
      inner = `<input class="rn-lit" data-lit="${i}" value="${esc(p.text || '')}" placeholder="KyNghi">`;
    } else if (p.kind === 'name') {
      inner = `<b>${t('rn.chip.name')}</b>`;
    } else if (p.kind === 'folder') {
      inner = `<b>${t('rn.chip.folder')}</b>`;
    } else if (p.kind === 'date') {
      inner = `<b>${t('rn.chip.date')}</b><select data-fmt="${i}">${
        DATE_FMTS.map((f) => `<option value="${f.v}"${f.v === p.format ? ' selected' : ''}>${t(f.k)}</option>`).join('')}</select>`;
    } else if (p.kind === 'counter') {
      inner = `<b>${t('rn.chip.counter')}</b><select data-w="${i}" title="${esc(t('rn.width'))}">${
        [1, 2, 3, 4, 5].map((w) => `<option value="${w}"${w === (p.width || 3) ? ' selected' : ''}>${w}</option>`).join('')}</select>`;
    }
    return `<span class="rn-chip">${inner}<button class="rn-x" data-del="${i}" title="${esc(t('rn.chip.remove'))}"><svg><use href="#i-x"/></svg></button></span>`;
  }).join('');

  $$('[data-lit]', box).forEach((el) => {
    el.oninput = debounce(() => { rnParts()[+el.dataset.lit].text = el.value; renameSample(); refreshPlan(); }, 400);
  });
  $$('[data-fmt]', box).forEach((el) => {
    el.onchange = () => { rnParts()[+el.dataset.fmt].format = el.value; renameSample(); refreshPlan(); };
  });
  $$('[data-w]', box).forEach((el) => {
    el.onchange = () => { rnParts()[+el.dataset.w].width = +el.value; renameSample(); refreshPlan(); };
  });
  $$('[data-del]', box).forEach((el) => {
    el.onclick = () => { rnParts().splice(+el.dataset.del, 1); renderRenameChips(); renameSample(); refreshPlan(); };
  });

  renameSample();
}

/** Ví dụ tên cũ → tên mới. Chỉ minh hoạ; sự thật nằm ở bước Kiểm tra. */
function renameSample() {
  const before = 'IMG_4821.jpg';
  const parts = rnParts();
  let stem = '';
  for (const p of parts) {
    if (p.kind === 'text') stem += p.text || '';
    else if (p.kind === 'name') stem += 'IMG_4821';
    else if (p.kind === 'folder') stem += currentLang() === 'en' ? 'Beach-Trip' : 'Da-Lat';
    else if (p.kind === 'date') stem += (DATE_FMTS.find((f) => f.v === p.format) || DATE_FMTS[0]).s;
    else if (p.kind === 'counter') stem += String(1).padStart(Math.min(Math.max(p.width || 3, 1), 5), '0');
  }
  const tr = (S.profile.rename && S.profile.rename.transforms) || {};
  if (tr.stripDiacritics) stem = stem.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd').replace(/Đ/g, 'D');
  if (tr.lowercase) stem = stem.toLowerCase();
  if (tr.kebab) stem = stem.replace(/\s+/g, '-');
  stem = stem.replace(/[<>:"/\\|?*]/g, '-').trim();
  stem = stem.replace(/^[_\-. ]+|[_\-. ]+$/g, '');   // cắt dấu ngăn cách lơ lửng, khớp lõi
  const after = (stem || 'IMG_4821') + '.jpg';
  $('#rn-before').textContent = before;
  $('#rn-after').textContent = after;
}

function addPart(kind) {
  const defaults = {
    text: { kind: 'text', text: '' },
    name: { kind: 'name' },
    folder: { kind: 'folder' },
    date: { kind: 'date', format: '%Y-%m-%d' },
    counter: { kind: 'counter', width: 3, start: 1 },
  };
  rnParts().push(defaults[kind]);
  renderRenameChips();
  refreshPlan();
  if (kind === 'text') setTimeout(() => $('.rn-chip:last-child .rn-lit')?.focus(), 30);
}

$$('#rn-add button').forEach((b) => { b.onclick = () => addPart(b.dataset.add); });

$('#rename-enabled').onchange = (e) => {
  if (!S.profile.rename) S.profile.rename = { inPlace: true, parts: [], transforms: {} };
  S.profile.rename.enabled = e.target.checked;
  syncControls();
  refreshPlan();
};
$('#rn-strip').onchange = (e) => { S.profile.rename.transforms.stripDiacritics = e.target.checked; renameSample(); refreshPlan(); };
$('#rn-lower').onchange = (e) => { S.profile.rename.transforms.lowercase = e.target.checked; renameSample(); refreshPlan(); };
$('#rn-kebab').onchange = (e) => { S.profile.rename.transforms.kebab = e.target.checked; renameSample(); refreshPlan(); };
$$('#rn-place button').forEach((b) => {
  b.onclick = () => {
    S.profile.rename.inPlace = b.dataset.place === 'in';
    $$('#rn-place button').forEach((x) => x.classList.toggle('active', x === b));
    refreshPlan();
  };
});
$('#sf-sidecar').onchange = (e) => { S.profile.safety.keepSidecarTogether = e.target.checked; refreshPlan(); };
$('#sf-project').onchange = (e) => {
  S.profile.safety.treatProjectFoldersAsUnit = e.target.checked;
  updateAccSummaries(); markPlanStale();
};
$('#sf-apps').onchange = (e) => {
  S.profile.safety.protectInstalledApps = e.target.checked;
  if (!e.target.checked) toast(t('de.appsOff'), 'warn', 6000);
  updateAccSummaries(); markPlanStale();
};
$('#sf-cloud').onchange = (e) => {
  S.profile.filters.skipCloudPlaceholder = e.target.checked;
  updateAccSummaries(); markPlanStale();
};
$('#sf-prefix').onchange = (e) => { S.profile.numberPrefix = e.target.checked; refreshPlan(); };

$('#btn-dest').onclick = async () => {
  const d = await invoke('pick_destination');
  if (!d) return;
  const src = S.sources[0] && S.sources[0].path;
  if (src) {
    const chk = await invoke('check_destination', { source: src, dest: d });
    if (!chk.ok) { toast(chk.reason, 'err', 6000); return; }
    if (chk.level === 'warn') toast(chk.reason, 'warn', 5000);
  }
  S.profile.destination = d;
  syncControls();
  refreshPlan();
};
$('#btn-dest-clear').onclick = () => { S.profile.destination = null; syncControls(); refreshPlan(); };
$('#btn-to-preview').onclick = () => go('preview');

// Phần tự chỉnh mặc định đóng — người mới chỉ thấy vài thẻ kiểu sắp xếp
$('#adv-toggle').onclick = () => {
  const btn = $('#adv-toggle');
  const open = btn.getAttribute('aria-expanded') === 'true';
  btn.setAttribute('aria-expanded', String(!open));
  $('#adv-block').classList.toggle('hidden', open);
};

/**
 * Mỗi mục nâng cao là một ngăn riêng, mặc định đóng, tiêu đề ghi sẵn giá trị đang
 * dùng. Trước đây mở "Tự chỉnh chi tiết" là xổ ra chừng 35 điều khiển cùng lúc
 * trong một cột hẹp — đúng chỗ người dùng thấy choáng nhất. Giờ mở ra là sáu dòng,
 * đọc lướt biết mình đang đặt gì, muốn sửa mục nào thì mở đúng mục đó.
 */
$$('[data-acc]').forEach((head) => {
  head.onclick = () => {
    const open = head.getAttribute('aria-expanded') === 'true';
    head.setAttribute('aria-expanded', String(!open));
    head.nextElementSibling.classList.toggle('hidden', open);
  };
});

function updateAccSummaries() {
  const p = S.profile;
  if (!p) return;

  const names = p.layers.map((l) => {
    const c = critById(splitSpec(l)[0]);
    return c ? c.name : splitSpec(l)[0];
  });
  $('#acc-sum-levels').textContent = names.length ? names.join(' › ') : t('acc.sum.none');

  $('#acc-sum-mode').textContent = t('de.mode.' + { Move: 'move', Copy: 'copy', ReportOnly: 'report', Hardlink: 'link' }[p.mode]);

  const d = p.duplicates;
  $('#acc-sum-dupes').textContent = !d.enabled ? t('acc.sum.off')
    : t('acc.sum.on') + ' · ' + t({ Quarantine: 'acc.dup.a1', Recycle: 'acc.dup.a2', Report: 'acc.dup.a3' }[d.action]);

  const rn = p.rename || {};
  $('#acc-sum-rename').textContent = rn.enabled
    ? t('acc.sum.rename', fmtNum((rn.parts || []).length)) : t('acc.sum.off');

  const on = [p.safety.keepSidecarTogether, p.safety.protectInstalledApps !== false,
    p.safety.treatProjectFoldersAsUnit, p.filters.skipCloudPlaceholder, p.numberPrefix].filter(Boolean).length;
  $('#acc-sum-safety').textContent = t('acc.sum.safety', on);

  $('#acc-sum-dest').textContent = p.destination ? tailPath(p.destination, 2) : t('acc.sum.dest.here');
}

// ── Cây cấu trúc xem nhanh
function renderTree(folders, opCount) {
  const box = $('#tree');
  if (!folders || !folders.length) {
    box.innerHTML = `<div class="tree-empty">${t('de.tree.empty')}</div>`;
    return;
  }
  // Cắt phần gốc chung để cây gọn
  const roots = S.sources.filter((s) => s.check.ok).map((s) => s.path.toLowerCase());
  const dest = (S.profile.destination || '').toLowerCase();
  const strip = (f) => {
    const l = f.toLowerCase();
    for (const r of [dest, ...roots]) {
      if (r && l.startsWith(r)) return f.slice(r.length).replace(/^[\\/]+/, '');
    }
    return f;
  };

  const tree = {};
  let shown = 0;
  for (const f of folders) {
    const rel = strip(f);
    if (!rel) continue;
    if (++shown > 1200) break;
    let node = tree;
    for (const part of rel.split(/[\\/]/).filter(Boolean)) {
      node[part] = node[part] || { __c: {} };
      node = node[part].__c;
    }
  }

  const lines = [];
  (function walk(node, depth, prefix) {
    const keys = Object.keys(node).sort();
    keys.forEach((k, i) => {
      const last = i === keys.length - 1;
      lines.push(
        `<div class="tree-row"><span class="tw">${prefix}${last ? '└─' : '├─'}</span>` +
        `<svg><use href="#i-folder"/></svg><span>${esc(k)}</span></div>`);
      if (depth < 5) walk(node[k].__c, depth + 1, prefix + (last ? '   ' : '│  '));
    });
  })(tree, 0, '');

  box.innerHTML = lines.join('');
  $('#tree-sub').textContent = t('de.tree.sub', fmtNum(folders.length), fmtNum(opCount));
}

// ── Lập kế hoạch (chạy lại mỗi khi đổi cấu hình)
const refreshPlan = debounce(async () => {
  if (!S.analytics) return;
  $('#tree').innerHTML = `<div class="tree-empty">${t('de.tree.calc')}</div>`;
  let res;
  try {
    res = await invoke('make_plan', { profile: S.profile });
  } catch (e) {
    $('#tree').innerHTML = `<div class="tree-empty">${esc(typeof e === 'string' ? e : String(e))}</div>`;
    return;
  }
  S.plan = res;
  S.deselected.clear();
  S.pages.clear();
  S.pending.clear();

  renderTree(res.folders, res.summary.total);
  renderWarnings(res.warnings, res.dupReport, res.nearReport);
  renderSummary();
  loadPage(0, true);
}, 340);

function renderWarnings(warnings, dup, near) {
  const box = $('#warn-box');
  let html = '';
  for (const w of warnings || []) {
    html += `<div class="issue warn"><svg><use href="#i-warn"/></svg><span>${esc(w)}</span></div>`;
  }
  if (dup && dup.totalExtras > 0) {
    html += `<div class="issue info"><svg><use href="#i-copy"/></svg><span>${t('warn.dupes', fmtNum(dup.totalExtras), fmtBytes(dup.totalWasted))}</span></div>`;
  }
  if (near && near.skippedTooMany) {
    html += `<div class="issue warn"><svg><use href="#i-warn"/></svg><span>${t('warn.nearTooMany')}</span></div>`;
  } else if (near && near.totalExtras > 0) {
    html += `<div class="issue info"><svg><use href="#i-copy"/></svg><span>${t('warn.near', fmtNum(near.totalExtras), fmtBytes(near.totalWasted))}</span></div>`;
  }
  box.innerHTML = html;
}

// ═══════════════════════════════════════════════════════════ 4. Màn Xem trước

/** Một câu tiếng Việt bình thường tóm tắt việc sắp làm */
function plainSummary() {
  if (!S.plan) return t('pv.calc');
  const s = S.plan.summary;
  const move = Math.max(0, s.moves - S.deselected.size);
  if (S.profile.mode === 'ReportOnly') return t('plain.report', fmtNum(s.total));

  // Đổi tên tại chỗ: nói "đổi tên" chứ không "chuyển vào thư mục mới"
  const rn = S.profile.rename;
  if (rn && rn.enabled && rn.inPlace !== false) {
    const bits = [t('plain.renameInPlace', fmtNum(Math.max(0, s.renames)))];
    if (s.duplicates) bits.push(t('plain.dupes', fmtNum(s.duplicates)));
    if (s.keeps) bits.push(t('plain.kept', fmtNum(s.keeps)));
    return bits.join('. ') + '. ' + t('plain.noDelete');
  }

  const bits = [t(S.profile.mode === 'Copy' ? 'plain.copy' : 'plain.move',
    fmtNum(move), fmtNum(s.newFolders))];
  if (s.renames) bits.push(t('plain.renamed', fmtNum(s.renames)));
  if (s.duplicates) bits.push(t('plain.dupes', fmtNum(s.duplicates)));
  if (s.keeps) bits.push(t('plain.kept', fmtNum(s.keeps)));
  if (S.profile.recursive && S.profile.safety.cleanEmptyDirs) bits.push(t('plain.sweep'));
  return bits.join('. ') + '. ' + t('plain.noDelete');
}

/**
 * Bước cuối lấy tóm tắt làm mặt tiền, danh sách làm phần phụ.
 *
 * Phần mềm đã hoàn tác được 100%, nên bắt người dùng duyệt hai nghìn dòng là đòi
 * một nghĩa vụ mà chính kiến trúc an toàn của nó đã gánh hộ. Ở đây họ đọc một câu,
 * liếc mấy con số, rồi bấm. Con số nào cũng bấm được để lọc thẳng vào nhóm đó,
 * nên ai muốn soi vẫn tới nơi bằng một cú bấm.
 */
function renderSummary() {
  if (!S.plan) return;
  const s = S.plan.summary;
  const off = S.deselected.size;
  $('#plain-summary').textContent = plainSummary();

  const chips = [
    { f: 'moved', b: fmtNum(s.moves), i: t('sum.moved') },
    { f: 'renamed', b: fmtNum(s.renames), i: t('sum.renamed') },
    { f: 'dup', b: fmtNum(s.duplicates), i: t('sum.dupes') },
    { f: 'keep', b: fmtNum(s.keeps), i: t('sum.kept') },
    { b: fmtNum(s.newFolders), i: t('sum.folders') },
    { b: fmtBytes(s.totalBytes), i: t('sum.bytes') },
  ];
  if (off) chips.push({ b: fmtNum(off), i: t('sum.unticked'), k: 'amber' });

  $('#pv-chips').innerHTML = chips.map((c) => (c.f
    ? `<button class="pv-chip ${S.filter === c.f ? 'on' : ''}" data-chip="${c.f}"><b>${c.b}</b><i>${esc(c.i)}</i></button>`
    : `<span class="pv-chip flat ${c.k || ''}"><b>${c.b}</b><i>${esc(c.i)}</i></span>`)).join('');

  $$('[data-chip]').forEach((b) => {
    b.onclick = () => { setFilter(b.dataset.chip); openList(true); };
  });

  syncListToggle();
}

function syncListToggle() {
  const n = S.listTotal || (S.plan && S.plan.summary.total) || 0;
  $('#btn-list-toggle').textContent = t(S.listOpen ? 'pv.hideList' : 'pv.showList', fmtNum(n));
  $('#btn-list-toggle').setAttribute('aria-expanded', String(S.listOpen));
}

function setFilter(f) {
  S.filter = f;
  $$('#filter-seg button').forEach((x) => {
    const on = x.dataset.f === f;
    x.classList.toggle('active', on);
    x.setAttribute('aria-pressed', String(on));
  });
  $('#vlist').scrollTop = 0;
  loadPage(0, true);
  renderSummary();
}

function openList(open) {
  S.listOpen = open;
  $('#list-block').classList.toggle('hidden', !open);
  syncListToggle();
  // Danh sách ảo đo chiều cao khung nhìn; lúc còn ẩn chiều cao bằng 0 nên phải
  // vẽ lại ngay sau khi nó hiện ra.
  if (open) requestAnimationFrame(drawRows);
}

$('#btn-list-toggle').onclick = () => openList(!S.listOpen);

/**
 * Bỏ tick cả nhóm đang lọc. Trước đây bộ lọc chỉ để nhìn: muốn để yên vài trăm
 * file trùng lặp thì phải bấm từng dòng một.
 */
$('#btn-bulk').onclick = async () => {
  const ids = await invoke('plan_ids', { filter: S.filter, search: S.search }).catch(() => []);
  if (!ids.length) return;
  const allOff = ids.every((id) => S.deselected.has(id));
  ids.forEach((id) => (allOff ? S.deselected.delete(id) : S.deselected.add(id)));
  toast(t(allOff ? 'pv.bulk.re' : 'pv.bulk.did', fmtNum(ids.length)), 'ok');
  drawRows();
  renderSummary();
  runPreflight();
};

/** [class, nhãn, lời giải thích khi rê chuột] — bảy loại nhãn mà không chú giải thì phải đoán */
function tagOf(op) {
  if (op.near) return ['tag-near', t('tag.near'), t('tag.near.tip')];
  if (op.action === 'Quarantine' || op.action === 'Recycle') return ['tag-dup', t('tag.dup'), t('tag.dup.tip')];
  if (op.action === 'Keep') return ['tag-keep', t('tag.keep'), t('tag.keep.tip')];
  if (op.action === 'Copy') return ['tag-copy', t('tag.copy'), t('tag.copy.tip')];
  if (op.action === 'Hardlink') return ['tag-copy', t('tag.link'), t('tag.link.tip')];
  if (op.renamed) return ['tag-ren', t('tag.rename'), t('tag.rename.tip')];
  return ['tag-move', t('tag.move'), t('tag.move.tip')];
}

async function loadPage(page, reset) {
  if (reset) { S.pages.clear(); S.pending.clear(); }
  if (S.pages.has(page) || S.pending.has(page)) return;
  S.pending.add(page);
  try {
    const res = await invoke('plan_page', {
      offset: page * PAGE, limit: PAGE, filter: S.filter, search: S.search,
    });
    S.pages.set(page, res.ops);
    S.listTotal = res.total;
    $('#list-count').textContent = t('pv.rows', fmtNum(res.total));
    $('#vlist-inner').style.height = (res.total * ROW_H) + 'px';
    syncListToggle();
    drawRows();
  } finally {
    S.pending.delete(page);
  }
}

function drawRows() {
  const vp = $('#vlist');
  const inner = $('#vlist-inner');
  const top = vp.scrollTop;
  const first = Math.max(0, Math.floor(top / ROW_H) - 6);
  const last = Math.min(S.listTotal, Math.ceil((top + vp.clientHeight) / ROW_H) + 6);

  for (let p = Math.floor(first / PAGE); p <= Math.floor(Math.max(0, last - 1) / PAGE); p++) loadPage(p);

  let html = '';
  for (let i = first; i < last; i++) {
    const op = S.pages.get(Math.floor(i / PAGE)) && S.pages.get(Math.floor(i / PAGE))[i % PAGE];
    if (!op) {
      html += `<div class="vrow" style="top:${i * ROW_H}px"><span class="muted sm">…</span></div>`;
      continue;
    }
    const [cls, label, tip] = tagOf(op);
    const off = S.deselected.has(op.id);
    const canToggle = op.action !== 'Keep';
    const destText = op.action === 'Recycle' ? t('tag.recycleBin') : tailPath(op.dest, 4);
    html += `
      <div class="vrow ${off ? 'off' : ''}" style="top:${i * ROW_H}px">
        <input type="checkbox" data-id="${op.id}" ${off || !canToggle ? '' : 'checked'} ${canToggle ? '' : 'disabled'}
               aria-label="${esc(t('pv.rowTick'))}: ${esc(baseName(op.src))}">
        <span class="from" title="${esc(op.src)}">${esc(baseName(op.src))}</span>
        <svg class="ar"><use href="#i-arrow"/></svg>
        <span class="to" title="${esc(op.dest)}">${esc(destText)}</span>
        <span class="tag ${cls}" title="${esc(tip)}">${label}</span>
        <span class="sz">${fmtBytes(op.size)}</span>
      </div>`;
  }
  inner.innerHTML = html;

  $$('input[data-id]', inner).forEach((cb) => {
    cb.onchange = () => {
      const id = +cb.dataset.id;
      if (cb.checked) S.deselected.delete(id); else S.deselected.add(id);
      cb.closest('.vrow').classList.toggle('off', !cb.checked);
      renderSummary();
      runPreflight();
    };
  });
}

$('#vlist').addEventListener('scroll', () => requestAnimationFrame(drawRows), { passive: true });

// Đổi cỡ cửa sổ làm thay đổi số dòng vừa khung — vẽ lại theo.
if (window.ResizeObserver) {
  let lastH = 0;
  new ResizeObserver((entries) => {
    const h = entries[0].contentRect.height;
    if (Math.abs(h - lastH) < 1) return;
    lastH = h;
    requestAnimationFrame(drawRows);
  }).observe($('#vlist'));
}

$$('#filter-seg button').forEach((b) => { b.onclick = () => setFilter(b.dataset.f); });

$('#search').oninput = debounce((e) => {
  S.search = e.target.value;
  $('#vlist').scrollTop = 0;
  loadPage(0, true);
}, 280);

// ── Kiểm tra trước khi chạy
const runPreflight = debounce(async () => {
  if (!S.plan) return;
  const box = $('#pf-box');
  box.innerHTML = `<div class="issue info"><svg><use href="#i-clock"/></svg><span>${t('pv.checking')}</span></div>`;
  let pf;
  try {
    pf = await invoke('run_preflight', { deselected: Array.from(S.deselected) });
  } catch (e) {
    box.innerHTML = `<div class="issue error"><svg><use href="#i-x"/></svg><span>${esc(String(e))}</span></div>`;
    $('#btn-apply').disabled = true;
    return;
  }
  S.skipIds = pf.skipIds || [];

  box.innerHTML = (pf.issues || []).map((i) => `
    <div class="issue ${i.level}">
      <svg><use href="#${i.level === 'error' ? 'i-x' : 'i-warn'}"/></svg>
      <span>${esc(i.msg)}</span>
    </div>`).join('');

  if (!pf.issues.length) {
    box.innerHTML = `<div class="issue info"><svg><use href="#i-check"/></svg><span>${t('pv.checkOk', fmtBytes(pf.estBytes))}</span></div>`;
  }
  $('#btn-apply').disabled = !pf.ok;
}, 420);

$('#btn-export').onclick = async () => {
  const path = await call('export_report', { format: 'html' });
  toast(t('toast.saved', path), 'ok', 6000);
  invoke('reveal', { path }).catch(() => {});
};

// ═══════════════════════════════════════════════════════════ 5. Chạy & kết quả

function showRun(title, cancellable) {
  $('#run-title').textContent = title;
  $('#run-note').textContent = '';
  $('#pbar').style.width = '0%';
  $('#run-count').textContent = '';
  $('#run-pct').textContent = '';
  $('#run-result').innerHTML = '';
  $('#run-actions').classList.toggle('hidden', !cancellable);
  go('run');
}

// Lõi phát sự kiện tiến trình cho TỪNG file — với 100.000 file thì ghi thẳng vào
// DOM mỗi lần là 100.000 lượt reflow. Gộp lại, mỗi khung hình chỉ vẽ một lần.
let progressPending = null;
let progressQueued = false;

function flushProgress() {
  progressQueued = false;
  const p = progressPending;
  if (!p) return;

  $('#run-note').textContent = p.note || '';
  const bar = $('#pbar');
  if (p.total > 0) {
    const pct = Math.min(100, (p.current / p.total) * 100);
    bar.style.opacity = '1';
    bar.style.width = pct.toFixed(1) + '%';
    $('#run-count').textContent = `${fmtNum(p.current)} / ${fmtNum(p.total)}`;
    $('#run-pct').textContent = pct.toFixed(0) + '%';
  } else if (p.current > 0) {
    // Chưa biết tổng số (đang quét) — chỉ đếm, không giả vờ có phần trăm
    $('#run-count').textContent = t('run.files', fmtNum(p.current));
    $('#run-pct').textContent = '';
    bar.style.width = '100%';
    bar.style.opacity = '.45';
  }
}

listen('foldu:progress', (e) => {
  progressPending = e.payload;
  if (progressQueued) return;
  progressQueued = true;
  requestAnimationFrame(flushProgress);
});

$('#btn-cancel').onclick = () => { invoke('cancel_run'); toast(t('run.stopping'), 'warn'); };

// Bước xác nhận: nói lại bằng lời thường việc sắp xảy ra, kèm lời trấn an
$('#btn-apply').onclick = () => {
  if (!S.plan) return;
  $('#confirm-text').textContent = plainSummary();
  $('#confirm-modal').classList.remove('hidden');
  setTimeout(() => $('#confirm-go').focus(), 40);
};
const closeConfirm = () => $('#confirm-modal').classList.add('hidden');
$('#confirm-close').onclick = closeConfirm;
$('#confirm-cancel').onclick = closeConfirm;
$('#confirm-modal').onclick = (e) => { if (e.target.id === 'confirm-modal') closeConfirm(); };
$('#confirm-go').onclick = () => { closeConfirm(); doApply(); };

async function doApply() {
  showRun(t('run.working'), true);
  let res;
  try {
    res = await invoke('apply_plan', {
      profileName: S.profile.name || 'Custom',
      deselected: Array.from(S.deselected),
      skipIds: S.skipIds,
    });
  } catch (e) {
    $('#run-title').textContent = t('run.failed');
    $('#run-result').innerHTML = `<div class="issue error"><svg><use href="#i-x"/></svg><span>${esc(String(e))}</span></div>`;
    $('#run-actions').classList.add('hidden');
    return;
  }

  S.lastSession = res.session;
  $('#run-title').textContent = t(res.cancelled ? 'run.stopped' : 'run.done');
  $('#run-actions').classList.add('hidden');
  $('#pbar').style.width = '100%';

  const errHtml = res.errors.length
    ? `<div class="card mt20"><div class="card-head"><h3>${t('res.errors')}</h3><span class="pill">${fmtNum(res.failed)}</span></div>
       <div class="tlist">${res.errors.map((e) => `
         <div class="tlist-row"><span class="n" title="${esc(e.path)}">${esc(baseName(e.path))}</span>
         <span class="s">${esc(e.msg)}</span></div>`).join('')}</div></div>`
    : '';

  $('#run-result').innerHTML = `
    <div class="result-stats">
      <div class="stat good"><b>${fmtNum(res.done)}</b><span>${t('res.done')}</span></div>
      <div class="stat"><b>${fmtBytes(res.bytes)}</b><span>${t('res.bytes')}</span></div>
      ${res.removedDirs ? `<div class="stat"><b>${fmtNum(res.removedDirs)}</b><span>${t('res.sweptDirs')}</span></div>` : ''}
      ${res.skipped ? `<div class="stat warn"><b>${fmtNum(res.skipped)}</b><span>${t('res.skipped')}</span></div>` : ''}
      ${res.failed ? `<div class="stat bad"><b>${fmtNum(res.failed)}</b><span>${t('res.failed')}</span></div>` : ''}
      <div class="stat"><b>${fmtDur(res.elapsedMs)}</b><span>${t('res.time')}</span></div>
    </div>
    <div class="row">
      <button class="btn btn-primary btn-lg" id="r-undo"><svg><use href="#i-undo"/></svg>${t('res.undo')}</button>
      <button class="btn" id="r-open"><svg><use href="#i-folder"/></svg>${t('res.open')}</button>
      <button class="btn btn-ghost" id="r-export"><svg><use href="#i-export"/></svg>${t('res.export')}</button>
    </div>${errHtml}`;

  $('#r-undo').onclick = () => doUndo(res.session);
  $('#r-open').onclick = () => {
    const p = S.profile.destination || (S.sources[0] && S.sources[0].path);
    if (p) invoke('reveal', { path: p }).catch(() => {});
  };
  $('#r-export').onclick = async () => {
    const path = await call('export_report', { format: 'html' });
    toast(t('toast.saved', path), 'ok', 6000);
  };

  toast(res.failed ? t('toast.doneErr', fmtNum(res.done), fmtNum(res.failed)) : t('toast.done', fmtNum(res.done)),
    res.failed ? 'warn' : 'ok');
  S.plan = null;
  S.sources = [];
  renderSources();
  renderDrives();
}

async function doUndo(session) {
  showRun(t('run.undoing'), false);
  let r;
  try {
    r = await invoke('undo', { session, seqs: null });
  } catch (e) {
    $('#run-title').textContent = t('run.undoFailed');
    $('#run-result').innerHTML = `<div class="issue error"><svg><use href="#i-x"/></svg><span>${esc(String(e))}</span></div>`;
    return;
  }
  $('#run-title').textContent = t('run.undone');
  const conflicts = r.conflicts.length
    ? `<div class="card mt20"><div class="card-head"><h3>${t('res.conflicts')}</h3><span class="pill">${fmtNum(r.conflicts.length)}</span></div>
       <div class="tlist">${r.conflicts.map((c) => `
         <div class="tlist-row"><span class="n" title="${esc(c.path)}">${esc(baseName(c.path))}</span>
         <span class="s">${esc(c.msg)}</span></div>`).join('')}</div></div>`
    : '';
  $('#run-result').innerHTML = `
    <div class="result-stats">
      <div class="stat good"><b>${fmtNum(r.restored)}</b><span>${t('res.restored')}</span></div>
      ${r.failed ? `<div class="stat bad"><b>${fmtNum(r.failed)}</b><span>${t('run.undoFailed')}</span></div>` : ''}
      ${r.restoredDirs ? `<div class="stat"><b>${fmtNum(r.restoredDirs)}</b><span>${t('res.restoredDirs')}</span></div>` : ''}
      <div class="stat"><b>${fmtNum(r.removedDirs)}</b><span>${t('res.removedDirs')}</span></div>
    </div>${conflicts}`;
  toast(t('toast.undone', fmtNum(r.restored)), 'ok');
  loadHistory();
}

// ═══════════════════════════════════════════════════════════ 6. Lịch sử

async function loadHistory() {
  const list = await invoke('history').catch(() => []);
  const box = $('#history-list');
  if (!list.length) {
    box.innerHTML = `<p class="muted">${t('hi.none')}</p>`;
    return;
  }
  const badge = { DONE: ['done'], UNDONE: ['undone'], RUNNING: ['running'],
                  PARTIAL: ['partial'], CANCELLED: ['partial'] };
  box.innerHTML = list.map((s) => {
    const cls = (badge[s.status] || ['undone'])[0];
    const text = t('st.' + s.status) === 'st.' + s.status ? s.status : t('st.' + s.status);
    return `
      <div class="hitem">
        <div class="top">
          <div>
            <div class="when">${fmtDate(s.startedAt)}</div>
            <div class="meta">${esc(s.profile || '—')} · ${t('hi.actions', fmtNum(s.done), fmtNum(s.total))}${s.failed ? ' · ' + t('hi.errors', fmtNum(s.failed)) : ''}</div>
          </div>
          <div class="row">
            <span class="badge ${cls}">${text}</span>
            ${s.canUndo ? `<button class="btn btn-ghost" data-undo="${esc(s.session)}"><svg><use href="#i-undo"/></svg>${t('hi.undo')}</button>` : ''}
          </div>
        </div>
        <div class="roots" title="${esc((s.roots || []).join(', '))}">${esc((s.roots || []).join(' · '))}</div>
      </div>`;
  }).join('');
  $$('[data-undo]', box).forEach((b) => { b.onclick = () => doUndo(b.dataset.undo); });
}

// ── Banner phiên bị ngắt
async function checkInterrupted() {
  const list = await invoke('interrupted').catch(() => []);
  if (!list.length) return;
  const s = list[0];
  $('#resume-text').textContent = t('resume.text', fmtDate(s.startedAt), fmtNum(s.done));
  $('#resume-banner').classList.remove('hidden');
  $('#resume-undo').onclick = () => doUndo(s.session);
  $('#resume-dismiss').onclick = () => $('#resume-banner').classList.add('hidden');
}

// ═══════════════════════════════════════════════════════════ 7. Cài đặt

function renderSettings() {
  const s = S.settings;
  if (!s) return;
  $('#groups-editor').innerHTML = s.groups.map((g, i) => `
    <div class="ed-row">
      <label>${esc(g.name)}</label>
      <input type="text" data-g="${i}" value="${esc(g.exts.join(' '))}">
    </div>`).join('');
  $('#keywords-editor').innerHTML = s.keywords.map((k, i) => `
    <div class="ed-row">
      <label>${esc(k.folder)}</label>
      <input type="text" data-k="${i}" value="${esc(k.words.join(', '))}">
    </div>`).join('');
  $('#noise-editor').value = s.noiseWords.join(' ');
  $('#ignore-editor').value = (S.profile.filters.ignorePatterns || []).join('\n');
}

$('#btn-save-settings').onclick = async () => {
  const s = S.settings;
  $$('[data-g]').forEach((inp) => {
    s.groups[+inp.dataset.g].exts = inp.value.split(/[\s,]+/).filter(Boolean).map((x) => x.toLowerCase());
  });
  $$('[data-k]').forEach((inp) => {
    s.keywords[+inp.dataset.k].words = inp.value.split(',').map((x) => x.trim()).filter(Boolean);
  });
  s.noiseWords = $('#noise-editor').value.split(/[\s,]+/).filter(Boolean);
  S.profile.filters.ignorePatterns = $('#ignore-editor').value.split('\n').map((x) => x.trim()).filter(Boolean);
  await call('set_settings', { s });
  toast(t('toast.settings'), 'ok');
  refreshPlan();
};

// ═════════════════════════════════════════════════════════════════ Khởi động

(async function init() {
  const [settings, presets, catalog, profile] = await Promise.all([
    invoke('get_settings'),
    invoke('get_presets'),
    invoke('get_catalog'),
    invoke('default_profile'),
  ]);
  S.settings = settings;
  S.presets = presets;
  S.catalog = catalog;
  S.profile = profile;

  document.documentElement.dataset.theme = settings.theme === 'light' ? 'light' : 'dark';

  // Ngôn ngữ phải áp trước khi vẽ bất cứ thứ gì
  const code = settings.lang === 'en' ? 'en' : 'vi';
  setLang(code);
  applyI18n();
  $('#flag-now').innerHTML = `<use href="#flag-${code}"/>`;
  $('#lang-code').textContent = code.toUpperCase();

  renderPresets();
  renderLayers();
  syncControls();
  renderRecent();
  renderDrives();
  readScope();

  if (settings.langPicked) checkInterrupted();
  else showLangPicker();   // lần đầu mở: hỏi ngôn ngữ trước, mọi thứ khác để sau

  syncAnFold();
  syncListToggle();

  document.addEventListener('keydown', (e) => {
    // Esc phải thoát được MỌI hộp thoại, không riêng hộp chọn tiêu chí. Riêng hộp
    // chọn ngôn ngữ lần đầu thì không: ở đó buộc phải chọn một thứ tiếng.
    if (e.key === 'Escape') {
      $('#crit-modal').classList.add('hidden');
      $('#confirm-modal').classList.add('hidden');
    }
    if (e.ctrlKey && e.key === 'o') { e.preventDefault(); $('#btn-pick').click(); }
  });
})();
