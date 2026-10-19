#!/usr/bin/env node
// Audit du contexte Claude Code d'un projet, à partir des transcripts. Aucune dépendance (Node 18+).
//
// Usage, depuis la racine du projet audité :
//   node <ce-dossier>/audit.mjs --name="Nom du projet" [--out=<dossier>] [dossier-transcripts]
//
//   --name    nom affiché ; défaut : 1er titre du README.md, sinon name du package.json, sinon nom du dossier
//   --out     dossier de sortie ; défaut : <ce-dossier>/<nom-du-projet>/
//   dossier   transcripts ; défaut : ~/.claude/projects/<slug du dossier courant>/
//
// Écrit dans --out : donnees.md (tableau, seuils, classement), context-table.excalidraw, context-chart.excalidraw.
//
// Contexte d'un appel = input + cache_read + cache_create : ce que le modèle relit à cet appel, pas un cumul.
// Chaque réponse compte une fois (dédup par message.id : Claude Code recopie le même usage sur chaque
// ligne d'une réponse en plusieurs blocs). Les transcripts des sous-agents sont inclus.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const opt = n => { const a = argv.find(x => x.startsWith(`--${n}=`)); return a ? a.slice(n.length + 3) : ''; };
const root = path.join(os.homedir(), '.claude/projects');
const dir = argv.find(a => !a.startsWith('--')) || path.join(root, process.cwd().replace(/[^A-Za-z0-9]/g, '-'));
if (!fs.existsSync(dir)) {
  console.error(`Dossier de transcripts introuvable : ${dir}\nRelance avec l'un de ceux-ci en argument :`);
  try { for (const d of fs.readdirSync(root)) console.error('  ' + path.join(root, d)); } catch {}
  process.exit(1);
}
const name = opt('name') || (() => {
  try { const h = fs.readFileSync('README.md', 'utf8').match(/^#\s+(.+)$/m); if (h) return h[1].trim(); } catch {}
  try { const n = JSON.parse(fs.readFileSync('package.json', 'utf8')).name; if (n) return n; } catch {}
  return path.basename(process.cwd());
})();
const out = opt('out') || path.join(HERE, name.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, ''));

// ---------- lecture des transcripts ----------
const CTX = {}; // famille -> { main: [], sub: [] } tailles de contexte par appel
const read = (file, s, side) => {
  const seen = {};
  for (const l of fs.readFileSync(file, 'utf8').split('\n')) {
    if (!l) continue;
    let e; try { e = JSON.parse(l); } catch { continue; }
    if (!side) {
      const ts = e.timestamp;
      if (ts) { if (!s.first || ts < s.first) s.first = ts; if (!s.last || ts > s.last) s.last = ts; }
      if (e.type === 'system' && e.subtype === 'compact_boundary') s.compacts++;
      if ((e.type === 'ai-title' || e.type === 'custom-title') && (e.aiTitle || e.customTitle)) s.title = e.customTitle || e.aiTitle;
      if (e.type === 'summary' && !s.title) s.title = e.summary;
      if (!s.prompt && e.type === 'user' && !e.isMeta) {
        let c = e.message && e.message.content;
        if (Array.isArray(c)) c = (c.find(x => x.type === 'text') || {}).text;
        if (typeof c === 'string' && !/^\s*</.test(c) && !c.startsWith('Caveat')) s.prompt = c;
      }
    }
    const m = e.message, u = m && m.usage;
    if (e.type !== 'assistant' || !u || !m.model || m.model === '<synthetic>') continue;
    if (m.id) { if (seen[m.id]) continue; seen[m.id] = 1; }
    const ctx = (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0);
    const fam = (m.model.match(/^claude-([a-z]+)/) || [, m.model])[1];
    (CTX[fam] || (CTX[fam] = { main: [], sub: [] }))[side ? 'sub' : 'main'].push(ctx);
    s.calls[fam] = (s.calls[fam] || 0) + 1;
    if (!side) { s.ctx.push(ctx); if (!(s.peak[fam] >= ctx)) s.peak[fam] = ctx; }
  }
};
const sessions = [];
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith('.jsonl')) continue;
  const id = f.slice(0, -6), s = { id, peak: {}, calls: {}, ctx: [], compacts: 0 };
  read(path.join(dir, f), s, false);
  const sd = path.join(dir, id, 'subagents');
  if (fs.existsSync(sd)) for (const g of fs.readdirSync(sd)) if (g.endsWith('.jsonl')) read(path.join(sd, g), s, true);
  if (s.ctx.length) sessions.push(s);
}
if (!sessions.length) { console.error(`Aucune session avec usage dans ${dir}`); process.exit(1); }

// ---------- statistiques ----------
const q = (a, p) => a[Math.min(a.length - 1, Math.max(0, Math.ceil(p * a.length) - 1))];
const stats = a => { a = [...a].sort((x, y) => x - y); return { n: a.length, min: a[0], p50: q(a, .5), mean: a.reduce((s, x) => s + x, 0) / a.length, p90: q(a, .9), max: a[a.length - 1] }; };
const K = x => Math.round(x / 1e3);
const k = x => K(x) + ' k';
const nb = x => String(x).replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
const fams = Object.keys(CTX).sort((a, b) => (CTX[b].main.length + CTX[b].sub.length) - (CTX[a].main.length + CTX[a].sub.length));
const allMain = sessions.flatMap(s => s.ctx), allSub = fams.flatMap(f => CTX[f].sub);
const peaks = sessions.map(s => Math.max(...s.ctx));
const MIN_SESSIONS = 3, MIN_SUB_CALLS = 100;

const rows = [[`Contexte max, ${nb(peaks.length)} sessions`, stats(peaks), false]];
for (const f of fams) { const p = sessions.map(s => s.peak[f]).filter(Boolean); if (p.length >= MIN_SESSIONS) rows.push([`dont ${f}, ${nb(p.length)} sessions`, stats(p), true]); }
rows.push([`Contexte par appel, fil principal (${nb(allMain.length)} appels)`, stats(allMain), false]);
if (allSub.length) rows.push([`Contexte par appel, sous-agents (${nb(allSub.length)} appels)`, stats(allSub), false]);

const over = t => allMain.filter(x => x > t).length;
const compacted = sessions.filter(s => s.compacts).length;
const first = sessions.reduce((a, s) => (s.first && s.first < a ? s.first : a), '9'), last = sessions.reduce((a, s) => (s.last && s.last > a ? s.last : a), '0');
const mainS = stats(allMain), subS = allSub.length ? stats(allSub) : null;
const pct200 = Math.round(100 * over(2e5) / allMain.length);

// ---------- donnees.md ----------
const label = s => (s.title || s.prompt || '?').replace(/\s+/g, ' ').replace(/\|/g, '/').slice(0, 70);
const dominant = s => { let b = null; for (const f in s.calls) if (!b || s.calls[f] > s.calls[b]) b = f; return b; };
const md = [];
md.push(`# Données de l'audit du contexte — ${name}`, '');
md.push(`Transcripts : \`${dir}\`  `, `Période : du ${first.slice(0, 10)} au ${last.slice(0, 10)}`, '');
md.push(`| ${name} | Min | Médiane | Moyenne | p90 | Max |`, '|---|--:|--:|--:|--:|--:|');
for (const [lab, t, sub] of rows) md.push(`| ${sub ? '&emsp;' : ''}${lab} | ${k(t.min)} | **${k(t.p50)}** | ${k(t.mean)} | ${k(t.p90)} | ${k(t.max)} |`);
md.push('', `Seuils : ${nb(over(2e5))} appels du fil principal dépassent 200 k (${pct200} %), ${nb(over(4e5))} dépassent 400 k, ${nb(over(8e5))} dépassent 800 k, sur ${nb(allMain.length)}.  `);
md.push(`Sessions compactées au moins une fois : ${compacted} sur ${sessions.length}.`);
if (subS) md.push(`Médiane fil principal / sous-agents : ${k(mainS.p50)} / ${k(subS.p50)}, soit ${Math.round(mainS.p50 / subS.p50)} fois moins dans un sous-agent.`);
md.push('', '## Contexte par appel, par famille', '', '| | appels | médiane | p90 | max |', '|---|--:|--:|--:|--:|');
for (const f of fams) if (CTX[f].main.length) { const t = stats(CTX[f].main); md.push(`| fil principal, ${f} | ${nb(t.n)} | ${k(t.p50)} | ${k(t.p90)} | ${k(t.max)} |`); }
for (const f of fams) if (CTX[f].sub.length) { const t = stats(CTX[f].sub); md.push(`| sous-agents, ${f} | ${nb(t.n)} | ${k(t.p50)} | ${k(t.p90)} | ${k(t.max)} |`); }
sessions.sort((x, y) => Math.max(...y.ctx) - Math.max(...x.ctx));
md.push('', '## Sessions au plus gros contexte max (top 25)', '', '| # | session | date | modèle | ctx max | ctx médian | appels | compacts | sujet |', '|--:|---|---|---|--:|--:|--:|--:|---|');
sessions.slice(0, 25).forEach((s, i) => { const t = stats(s.ctx); md.push(`| ${i + 1} | \`${s.id.slice(0, 8)}\` | ${(s.first || '').slice(0, 10)} | ${dominant(s)} | ${k(t.max)} | ${k(t.p50)} | ${t.n} | ${s.compacts} | ${label(s)} |`); });

// ---------- mini constructeur Excalidraw ----------
const scene = () => {
  const els = []; let seed = 100000;
  const base = (type, o) => {
    const e = { id: `e${els.length}`, type, x: 0, y: 0, width: 0, height: 0, angle: 0, strokeColor: '#1e1e1e', backgroundColor: 'transparent', fillStyle: 'solid', strokeWidth: 1, strokeStyle: 'solid', roughness: 1, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: ++seed, version: 1, versionNonce: ++seed, isDeleted: false, boundElements: [], updated: 1717689600000, link: null, locked: false, index: '' };
    Object.assign(e, o); els.push(e); return e;
  };
  const dims = (t, fs, font) => ({ w: Math.ceil(Math.max(...t.split('\n').map(l => l.length)) * fs * (font === 7 ? 0.7 : 0.6)), h: Math.ceil(t.split('\n').length * fs * 1.25) });
  const text = (x, y, t, { fs = 20, font = 7, color = '#1e1e1e', align = 'left', containerId = null } = {}) => {
    const { w, h } = dims(t, fs, font);
    const e = base('text', { x, y, width: w, height: h, strokeColor: color });
    Object.assign(e, { text: t, fontSize: fs, fontFamily: font, textAlign: align, verticalAlign: containerId ? 'middle' : 'top', baseline: fs - 2, lineHeight: 1.25, containerId, originalText: t, autoResize: true });
    return e;
  };
  const rect = (x, y, w, h, o = {}) => base('rectangle', { x, y, width: w, height: h, ...o });
  const line = (x, y1, y2, o = {}) => base('line', { x, y: y1, width: 0, height: y2 - y1, points: [[0, 0], [0, y2 - y1]], lastCommittedPoint: null, startBinding: null, endBinding: null, startArrowhead: null, endArrowhead: null, ...o });
  // boîte avec texte lié (le texte suit la boîte dans l'app)
  const box = (x, y, w, h, t, { bg = 'transparent', fs = 20, font = 7, align = 'center', color = '#1e1e1e', pad = 20, ...o } = {}) => {
    const r = rect(x, y, w, h, { backgroundColor: bg, ...o });
    const d = dims(t, fs, font);
    const tx = text(align === 'left' ? x + pad : x + (w - d.w) / 2, y + (h - d.h) / 2, t, { fs, font, color, align, containerId: r.id });
    r.boundElements = [{ type: 'text', id: tx.id }];
    return r;
  };
  const save = file => {
    els.forEach((e, i) => { e.index = String.fromCharCode(97 + Math.floor(i / 36)) + (i % 36 < 10 ? String(i % 36) : String.fromCharCode(55 + (i % 36))); });
    fs.writeFileSync(file, JSON.stringify({ type: 'excalidraw', version: 2, source: 'claude-context-audit', elements: els, appState: { viewBackgroundColor: '#ffffff', gridSize: 20, gridStep: 5, gridModeEnabled: false, lockedMultiSelections: {} }, files: {} }, null, 2) + '\n');
  };
  return { text, rect, line, box, save };
};

fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'donnees.md'), md.join('\n') + '\n');

// ---------- tableau ----------
{
  const S = scene(), X = 40, Y = 120, H = 60, W = 160;
  const W0 = Math.max(560, Math.ceil((Math.max(...rows.map(r => r[0].length + (r[2] ? 3 : 0)), name.length) * 14 + 60) / 20) * 20);
  S.text(40, 40, `Contexte Claude Code — ${name}`, { fs: 28, font: 5 });
  const line = (y, cells, bgs, sub) => cells.forEach((c, j) => S.box(j ? X + W0 + (j - 1) * W : X, y, j ? W : W0, H, c,
    { bg: bgs[j] || 'transparent', align: j ? 'center' : 'left', pad: sub && !j ? 60 : 20, color: sub && !j ? '#495057' : '#1e1e1e' }));
  line(Y, [name, 'Min', 'Médiane', 'Moyenne', 'p90', 'Max'], ['#ffec99', '#ffec99', '#a5d8ff', '#ffec99', '#ffec99', '#ffec99']);
  rows.forEach(([lab, t, sub], i) => line(Y + (i + 1) * H, [lab, k(t.min), k(t.p50), k(t.mean), k(t.p90), k(t.max)], ['', '', '#d0ebff'], sub));
  S.text(X, Y + (rows.length + 1) * H + 30,
    "Contexte d'un appel = ce que le modèle relit à cet appel (entrée + lecture et écriture de cache), pas un cumul.\n" +
    `Appels au-delà de 200 k : ${nb(over(2e5))}, 400 k : ${nb(over(4e5))}, 800 k : ${nb(over(8e5))}. Sessions compactées : ${compacted}/${sessions.length}.`,
    { fs: 16, font: 5, color: '#495057' });
  S.save(path.join(out, 'context-table.excalidraw'));
}

// ---------- diagramme ----------
{
  const S = scene(), X0 = 220, px = v => X0 + K(v); // 1 px = 1 k tokens, axe 0 → 1 M
  const t = (x, y, s, o = {}) => S.text(x, y, s, { fs: 20, ...o });
  S.text(40, 40, `${name} : contexte relu à chaque appel`, { fs: 28, font: 5 });
  S.rect(40, 100, 40, 24, { backgroundColor: '#4dabf7' }); t(90, 102, 'médiane');
  S.rect(220, 100, 40, 24, { backgroundColor: '#a5d8ff' }); t(270, 102, "jusqu'au p90");
  S.line(460, 96, 128, { strokeWidth: 4 }); t(476, 102, 'pic');
  const AX = 170, TOP = AX + 50, PITCH = 70, H = 40;
  for (const v of [0, 200, 400, 600, 800, 1000]) { S.line(X0 + v, AX + 26, AX + 40, { strokeColor: '#868e96' }); t(X0 + v - (v ? 20 : 5), AX, v ? (v === 1000 ? '1 M' : v + ' k') : '0', { fs: 16, color: '#868e96' }); }
  const mainRows = [['toutes', mainS], ...fams.filter(f => CTX[f].main.length && sessions.filter(s => s.peak[f]).length >= MIN_SESSIONS).map(f => [f, stats(CTX[f].main)])];
  const subRows = fams.filter(f => CTX[f].sub.length >= MIN_SUB_CALLS).map(f => [f, stats(CTX[f].sub)]);
  const group = (title, y0, fill, light, data) => {
    S.text(40, y0, title, { fs: 28, font: 5 });
    data.forEach(([lab, st], i) => {
      const y = y0 + 44 + i * PITCH, p50 = K(st.p50), p90 = K(st.p90), max = K(st.max);
      t(40, y + 9, lab);
      S.rect(X0, y, p50, H, { backgroundColor: fill });
      S.rect(X0 + p50, y, p90 - p50, H, { backgroundColor: light });
      const m = `${p50} k`, w = m.length * 14, inside = p50 >= w + 10;
      if (inside) t(X0 + p50 - w - 8, y + 10, m);
      S.line(X0 + max, y - 4, y + H + 4, { strokeWidth: 4 });
      const ml = `${max} k`, right = max <= 850;
      t(right ? X0 + max + 16 : X0 + max - 12 - ml.length * 14, y + 10, ml);
      if (!inside) t(X0 + max + 16 + (ml.length + 2) * 14, y + 12, `(médiane ${m})`, { fs: 16, color: '#495057' });
    });
    return y0 + 44 + data.length * PITCH;
  };
  let y = group('Fil principal', TOP + 10, '#4dabf7', '#a5d8ff', mainRows);
  if (subRows.length) y = group('Sous-agents', y + 40, '#40c057', '#b2f2bb', subRows);
  S.line(X0 + 200, TOP, y, { strokeColor: '#e8590c', strokeWidth: 2, strokeStyle: 'dashed' });
  S.line(X0 + 1000, TOP, y, { strokeColor: '#e03131', strokeWidth: 2 });
  S.text(X0 + 210, TOP + 14, '/compact conseillé', { fs: 16, font: 5, color: '#e8590c' });
  S.text(X0 + 850, TOP + 14, 'limite du modèle', { fs: 16, font: 5, color: '#e03131' });
  const facts = [
    `${pct200} % des appels du fil principal relisent plus de 200 k`,
    `${compacted} session${compacted > 1 ? 's' : ''} sur ${sessions.length} compactée${compacted > 1 ? 's' : ''}`,
    ...(subS ? [`En médiane, un sous-agent relit ${Math.round(mainS.p50 / subS.p50)} fois moins que le fil principal`] : []),
  ].join('\n');
  S.box(40, y + 40, 1200, Math.ceil((facts.split('\n').length * 25 + 60) / 20) * 20, facts, { bg: '#ffec99', font: 5, strokeWidth: 2, roundness: { type: 3 } });
  S.save(path.join(out, 'context-chart.excalidraw'));
}

console.log(md.join('\n'));
console.error(`\nÉcrit dans ${out} : donnees.md, context-table.excalidraw, context-chart.excalidraw`);
