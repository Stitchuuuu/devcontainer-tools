#!/usr/bin/env node
// Audit des tokens Claude Code d'un projet, depuis les transcripts (vérité terrain).
// Usage : node token-audit.js [dossier-projet]   (défaut : ~/.claude/projects/<slug du cwd>)
// Contexte = in + cache_read + cache_create d'UN appel (ce que le modèle relit). Dédup par message.id, sous-agents inclus.
const fs = require('fs'), path = require('path'), os = require('os');
const dir = process.argv[2] || path.join(os.homedir(), '.claude/projects', process.cwd().replace(/[^A-Za-z0-9]/g, '-'));
if (!fs.existsSync(dir)) { console.error('Dossier introuvable : ' + dir + '\nDossiers dispo :\n' + fs.readdirSync(path.join(os.homedir(), '.claude/projects')).join('\n')); process.exit(1); }

const CTX = {}; // famille -> tailles de contexte par appel
const read = (file, s, side) => {
  const seen = {};
  for (const l of fs.readFileSync(file, 'utf8').split('\n')) {
    if (!l) continue; let e; try { e = JSON.parse(l); } catch { continue; }
    if (!side) {
      if (e.timestamp) { if (!s.first || e.timestamp < s.first) s.first = e.timestamp; if (!s.last || e.timestamp > s.last) s.last = e.timestamp; }
      if (e.type === 'system' && e.subtype === 'compact_boundary') s.compacts++;
      if ((e.type === 'ai-title' || e.type === 'custom-title') && (e.aiTitle || e.customTitle)) s.title = e.customTitle || e.aiTitle;
      if (e.type === 'summary' && !s.title) s.title = e.summary;
      if (!s.prompt && e.type === 'user' && !e.isMeta) { let c = e.message && e.message.content; if (Array.isArray(c)) c = (c.find(x => x.type === 'text') || {}).text; if (typeof c === 'string' && !/^\s*</.test(c) && !c.startsWith('Caveat')) s.prompt = c; }
    }
    const m = e.message, u = m && m.usage;
    if (e.type !== 'assistant' || !u || !m.model || m.model === '<synthetic>') continue;
    if (m.id) { if (seen[m.id]) continue; seen[m.id] = 1; }
    const ctx = (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0);
    const fam = (m.model.match(/^claude-([a-z]+)/) || [, m.model])[1];
    const r = s.models[fam] || (s.models[fam] = { in: 0, cr: 0, cc: 0, out: 0 });
    r.in += u.input_tokens || 0; r.cr += u.cache_read_input_tokens || 0; r.cc += u.cache_creation_input_tokens || 0; r.out += u.output_tokens || 0;
    (CTX[fam] || (CTX[fam] = { main: [], sub: [] }))[side ? 'sub' : 'main'].push(ctx);
    if (side) s.subTot += ctx + (u.output_tokens || 0); else { s.ctx.push(ctx); }
  }
};

const sessions = [];
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith('.jsonl')) continue;
  const id = f.slice(0, -6), s = { id, models: {}, ctx: [], compacts: 0, subTot: 0, subN: 0 };
  read(path.join(dir, f), s, false);
  const sd = path.join(dir, id, 'subagents');
  if (fs.existsSync(sd)) for (const g of fs.readdirSync(sd)) if (g.endsWith('.jsonl')) { s.subN++; read(path.join(sd, g), s, true); }
  s.tot = 0; s.out = 0; for (const k in s.models) { const r = s.models[k]; r.tot = r.in + r.cr + r.cc + r.out; s.tot += r.tot; s.out += r.out; }
  if (s.tot > 0) sessions.push(s);
}
if (!sessions.length) { console.log('Aucune session avec usage dans ' + dir); process.exit(0); }

const si = x => x >= 1e9 ? (x / 1e9).toFixed(2) + 'G' : x >= 1e6 ? (x / 1e6).toFixed(1) + 'M' : x >= 1e3 ? (x / 1e3).toFixed(1) + 'k' : String(Math.round(x));
const q = (a, p) => a[Math.min(a.length - 1, Math.max(0, Math.ceil(p * a.length) - 1))];
const stats = a => { a = [...a].sort((x, y) => x - y); return { n: a.length, min: a[0], p50: q(a, .5), mean: a.reduce((s, x) => s + x, 0) / a.length, p90: q(a, .9), max: a[a.length - 1] }; };
const cell = s => `${s.n} | ${si(s.min)} | ${si(s.p50)} | ${si(s.mean)} | ${si(s.p90)} | ${si(s.max)}`;
const label = s => (s.title || s.prompt || '?').replace(/\s+/g, ' ').replace(/\|/g, '/').slice(0, 60);
const dominant = s => { let b = null; for (const k in s.models) if (!b || s.models[k].tot > s.models[b].tot) b = k; return b; };
const sum = (f) => { const o = { cr: 0, cc: 0 }; for (const s of sessions) for (const k in s.models) if (!f || k === f) { o.cr += s.models[k].cr; o.cc += s.models[k].cc; } return o; };
const fams = Object.keys(CTX).sort((a, b) => (CTX[b].main.length + CTX[b].sub.length) - (CTX[a].main.length + CTX[a].sub.length));
const hdr = '| | appels | min | médiane | moyenne | p90 | max |\n|---|--:|--:|--:|--:|--:|--:|';
const allMain = sessions.flatMap(s => s.ctx);
console.log(`# Audit contexte — ${dir}\n\n${sessions.length} sessions. Contexte = in + cache_read + cache_create d'un seul appel.\n`);

console.log('## Contexte par appel — fil principal\n\n' + hdr);
console.log('| **toutes** | ' + cell(stats(allMain)) + ' |');
for (const f of fams) if (CTX[f].main.length) console.log(`| ${f} | ` + cell(stats(CTX[f].main)) + ' |');
console.log('\n## Contexte par appel — sous-agents\n\n' + hdr);
for (const f of fams) if (CTX[f].sub.length) console.log(`| ${f} | ` + cell(stats(CTX[f].sub)) + ' |');

console.log('\n## Pic de contexte par session (fil principal)\n\n| | sessions | min | médiane | moyenne | p90 | max |\n|---|--:|--:|--:|--:|--:|--:|');
const peaks = sessions.filter(s => s.ctx.length).map(s => Math.max(...s.ctx));
console.log('| **toutes** | ' + cell(stats(peaks)) + ' |');
const T = [2e5, 4e5, 8e5];
console.log(`\nAppels du fil principal au-delà de ${T.map(t => si(t) + ' : ' + allMain.filter(x => x > t).length).join(', ')} (sur ${allMain.length}). Sessions avec au moins un /compact : ${sessions.filter(s => s.compacts).length}/${sessions.length}.`);

console.log('\n## Total cache (cumul, pour mémoire)\n\n| | lecture cache | écriture cache |\n|---|--:|--:|');
const a = sum(); console.log(`| **toutes** | ${si(a.cr)} | ${si(a.cc)} |`);
for (const f of fams) { const o = sum(f); console.log(`| ${f} | ${si(o.cr)} | ${si(o.cc)} |`); }

sessions.sort((x, y) => Math.max(0, ...y.ctx) - Math.max(0, ...x.ctx));
console.log('\n## Classement par pic de contexte (top 25)\n\n| # | session | date | modèle | ctx max | ctx médian | ctx p90 | appels | compacts | lecture cache | sujet |\n|--:|---|---|---|--:|--:|--:|--:|--:|--:|---|');
sessions.slice(0, 25).forEach((s, i) => { const c = s.ctx.length ? stats(s.ctx) : { p50: 0, p90: 0, max: 0 }; let cr = 0; for (const k in s.models) cr += s.models[k].cr;
  console.log(`| ${i + 1} | \`${s.id.slice(0, 8)}\` | ${(s.first || '').slice(0, 10)} | ${dominant(s)} | ${si(c.max)} | ${si(c.p50)} | ${si(c.p90)} | ${s.ctx.length} | ${s.compacts} | ${si(cr)} | ${label(s)} |`); });
