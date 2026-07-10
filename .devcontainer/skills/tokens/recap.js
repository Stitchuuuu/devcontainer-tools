#!/usr/bin/env node
// tokens skill — recap CLI. Args mode (cron/scripting) or interactive TTY menu.
// Reads local <project-root>/.claude/tokens/logs/YYYY-MM/*.jsonl.
// Node >= 18, zero deps.

const { parseArgs } = require('node:util');
const readline = require('node:readline');
const path = require('node:path');
const { compact, compactUSD } = require('./lib/format.js');
const { sinceReset, currentWeek, currentMonth, lastN, fromTo, all } = require('./lib/window.js');
const { findProjectRoot, readConfig, walkEvents } = require('./lib/logs.js');

const HELP = `
Usage: node recap.js [flags]

Window (mutually exclusive; default: --since-reset):
  --since-reset          since last Saturday 20h UTC (Anthropic weekly reset)
  --week                 current week (Mon 00h UTC)
  --month                current month (day 1 00h UTC)
  --last=<N>[dh]         sliding window (e.g. 7d, 24h)
  --from=YYYY-MM-DD      explicit start (optionally with --to=YYYY-MM-DD)
  --all                  all-time

Filter / group:
  --project=<slug>       filter by project title (repeatable)
  --current-project      current cwd project only (default)
  --by-project           aggregate per project
  --by-session           one row per session
  --by-day               one row per day (UTC)
  --by-model             one row per model
  --json                 machine output
  --no-color             disable ANSI
  --no-interactive       skip menu even on a TTY

Sans args + TTY: menu interactif.
`.trim();

function parseFlags(argv) {
  return parseArgs({
    args: argv,
    strict: true,
    allowPositionals: false,
    options: {
      'since-reset': { type: 'boolean' },
      week: { type: 'boolean' },
      month: { type: 'boolean' },
      last: { type: 'string' },
      from: { type: 'string' },
      to: { type: 'string' },
      all: { type: 'boolean' },
      project: { type: 'string', multiple: true },
      'all-projects': { type: 'boolean' },
      'current-project': { type: 'boolean' },
      'by-project': { type: 'boolean' },
      'by-session': { type: 'boolean' },
      'by-day': { type: 'boolean' },
      'by-model': { type: 'boolean' },
      json: { type: 'boolean' },
      'no-color': { type: 'boolean' },
      'no-interactive': { type: 'boolean' },
      help: { type: 'boolean', short: 'h' },
    },
  });
}

function resolveWindow(v) {
  const flags = ['since-reset', 'week', 'month', 'last', 'from', 'all'];
  const active = flags.filter(f => v[f]);
  if (active.length > 1) throw new Error(`window flags are mutually exclusive: ${active.join(', ')}`);
  if (v.all) return all();
  if (v.week) return currentWeek();
  if (v.month) return currentMonth();
  if (v.last) return lastN(v.last);
  if (v.from) return fromTo(v.from, v.to);
  return sinceReset();
}

function resolveGrouping(v, events) {
  const explicit = ['by-project', 'by-session', 'by-day', 'by-model'].filter(f => v[f]);
  if (explicit.length > 1) throw new Error(`--by-* flags are mutually exclusive: ${explicit.join(', ')}`);
  if (explicit.length === 1) return explicit[0].replace(/^by-/, '');
  const projects = new Set(events.map(e => e.project_root));
  const auto = projects.size > 1 ? 'project' : 'session';
  process.stderr.write(`grouping: ${auto} (auto — ${projects.size} project${projects.size > 1 ? 's' : ''})\n`);
  return auto;
}

function aggregate(events, groupBy, cfg) {
  const rows = new Map();
  const emptyTokens = () => ({ in: 0, cache_read: 0, cache_create: 0, out: 0 });
  for (const ev of events) {
    let key, label;
    if (groupBy === 'project') {
      key = ev.project_root;
      label = cfg && cfg.title ? formatProjectLabel(cfg) : ev.project_root;
    } else if (groupBy === 'session') {
      key = ev.session;
      label = ev.session;
    } else if (groupBy === 'day') {
      key = ev.ts_iso.slice(0, 10);
      label = key;
    } else if (groupBy === 'model') {
      key = ev.model;
      label = ev.model;
    }
    if (!rows.has(key)) {
      rows.set(key, {
        label,
        sessions: new Set(),
        models: new Set(),
        tokens: emptyTokens(),
        cost_usd: 0,
      });
    }
    const r = rows.get(key);
    r.sessions.add(ev.session);
    r.models.add(ev.model);
    r.tokens.in += ev.tokens.in;
    r.tokens.cache_read += ev.tokens.cache_read;
    r.tokens.cache_create += ev.tokens.cache_create;
    r.tokens.out += ev.tokens.out;
    r.cost_usd += ev.cost_usd;
  }
  return [...rows.values()].map(r => ({
    label: r.label,
    sessions: r.sessions.size,
    model: r.models.size === 1 ? [...r.models][0] : '—',
    tokens: r.tokens,
    cost_usd: r.cost_usd,
  }));
}

function totalRow(rows, groupBy) {
  if (rows.length <= 1) return null;
  const t = { in: 0, cache_read: 0, cache_create: 0, out: 0 };
  const sessions = new Set();
  const models = new Set();
  let cost = 0;
  for (const r of rows) {
    t.in += r.tokens.in;
    t.cache_read += r.tokens.cache_read;
    t.cache_create += r.tokens.cache_create;
    t.out += r.tokens.out;
    cost += r.cost_usd;
    for (const m of (r.model === '—' ? [] : [r.model])) models.add(m);
  }
  // sessions total: sum for by-session (each row = 1 unique session);
  // otherwise sum-of-uniques is a lower bound — safe: max across rows.
  const sessionsTotal = groupBy === 'session'
    ? rows.length
    : rows.reduce((s, r) => s + r.sessions, 0);
  return {
    label: `Total (${rows.length} ${pluralize(groupBy)})`,
    sessions: sessionsTotal,
    model: models.size === 1 ? [...models][0] : '—',
    tokens: t,
    cost_usd: cost,
  };
}

function pluralize(groupBy) {
  return { project: 'projects', session: 'sessions', day: 'days', model: 'models' }[groupBy] || groupBy;
}

function formatProjectLabel(cfg) {
  if (!cfg) return '—';
  if (cfg.subtitle) return `${cfg.title} — ${cfg.subtitle}`;
  return cfg.title;
}

function truncate(s, w) {
  if (s.length <= w) return s;
  return s.slice(0, Math.max(0, w - 1)) + '…';
}

// C:R = cache_read, C:W = cache_create (cache write). Kept compact for tables.
const HEADERS = ['Project', 'Sessions', 'Model', 'in', 'C:R', 'C:W', 'out', 'USD'];
const HEADERS_BY = {
  project: 'Project',
  session: 'Session',
  day: 'Day',
  model: 'Model',
};
const NUMERIC_COLS = new Set([1, 3, 4, 5, 6, 7]);

function renderTable(rows, groupBy, useColor) {
  if (rows.length === 0) return '';
  const headers = [...HEADERS];
  headers[0] = HEADERS_BY[groupBy] || 'Project';

  const dataRows = rows.map(r => [
    r.label,
    String(r.sessions),
    r.model,
    compact(r.tokens.in),
    compact(r.tokens.cache_read),
    compact(r.tokens.cache_create),
    compact(r.tokens.out),
    compactUSD(r.cost_usd),
  ]);

  // Truncate label col if longer than 32 chars.
  for (const row of dataRows) row[0] = truncate(row[0], 32);

  const widths = headers.map((h, i) => Math.max(h.length, ...dataRows.map(r => r[i].length)));

  const pad = (cell, i) => NUMERIC_COLS.has(i) ? cell.padStart(widths[i]) : cell.padEnd(widths[i]);
  // Separator matches the visible block width around each cell: ` ${padded} ` = w + 2 chars.
  const sep = widths.map((w, i) => {
    const total = w + 2;
    const dashes = '-'.repeat(Math.max(3, total - 1));
    return NUMERIC_COLS.has(i) ? dashes + ':' : ':' + dashes;
  });

  const bold = (s) => useColor ? `\x1b[1m${s}\x1b[0m` : s;
  const dim = (s) => useColor ? `\x1b[2m${s}\x1b[0m` : s;

  const headerRow = '| ' + headers.map((h, i) => bold(pad(h, i))).join(' | ') + ' |';
  const sepRow = '|' + sep.join('|') + '|';
  const dataLines = dataRows.map((r, idx) => {
    const isTotal = idx === dataRows.length - 1 && rows[idx].label.startsWith('Total (');
    const line = '| ' + r.map((c, i) => pad(c, i)).join(' | ') + ' |';
    return isTotal ? dim(line) : line;
  });

  return [headerRow, sepRow, ...dataLines].join('\n');
}

function collectEvents(projectRoot, window) {
  const events = [];
  for (const ev of walkEvents(projectRoot, window)) events.push(ev);
  return events;
}

function runStats(v) {
  const projectRoot = findProjectRoot();
  const cfg = readConfig(projectRoot);
  const window = resolveWindow(v);

  const events = collectEvents(projectRoot, window);

  // --project filter (session 2 scope: match current cwd project's title)
  let filteredEvents = events;
  if (v.project && v.project.length) {
    const title = cfg ? cfg.title : null;
    const matches = v.project.some(p => p === title);
    if (!matches) {
      process.stdout.write(`No sessions matching --project=${v.project.join(',')} in this project (title=${title || '<no config>'}).\n`);
      return;
    }
  }

  const groupBy = resolveGrouping(v, filteredEvents);
  const rows = aggregate(filteredEvents, groupBy, cfg);
  rows.sort((a, b) => b.cost_usd - a.cost_usd);
  const total = totalRow(rows, groupBy);
  if (total) rows.push(total);

  if (v.json) {
    const out = rows.map(r => ({
      label: r.label,
      sessions: r.sessions,
      model: r.model,
      tokens: r.tokens,
      cost_usd: Number(r.cost_usd.toFixed(6)),
    }));
    process.stdout.write(JSON.stringify({ window: { label: window.label, start: new Date(window.startEpoch).toISOString(), end: new Date(window.endEpoch).toISOString() }, groupBy, rows: out }, null, 2) + '\n');
    return;
  }

  if (filteredEvents.length === 0) {
    process.stdout.write(`No sessions logged in this window (${window.label}).\n`);
    return;
  }

  const useColor = process.stdout.isTTY && !v['no-color'];
  process.stdout.write(`Window: ${window.label} (${new Date(window.startEpoch).toISOString()} → ${new Date(window.endEpoch).toISOString()})\n`);
  process.stdout.write(`Legend: C:R = cache read, C:W = cache write (cache creation).\n`);
  process.stdout.write(renderTable(rows, groupBy, useColor) + '\n');
}

function ask(rl, q) {
  return new Promise(resolve => rl.question(q, resolve));
}

async function menuLoop() {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  process.on('SIGINT', () => { rl.close(); process.exit(130); });
  try {
    while (true) {
      process.stdout.write('\n=== Tokens recap ===\n');
      process.stdout.write('1. View stats\n');
      process.stdout.write('2. Manage projects (coming in session 4)\n');
      process.stdout.write('3. Manage models (coming in session 3)\n');
      process.stdout.write('4. Refresh pricing (coming in session 3)\n');
      process.stdout.write('5. Quit\n');
      const choice = (await ask(rl, '> ')).trim().toLowerCase();
      if (choice === '5' || choice === 'q' || choice === 'quit' || choice === '') break;
      if (choice === '2') { process.stdout.write('  « coming in session 4 »\n'); continue; }
      if (choice === '3' || choice === '4') { process.stdout.write('  « coming in session 3 »\n'); continue; }
      if (choice === '1') {
        await viewStatsInteractive(rl);
        continue;
      }
      process.stdout.write(`  unknown choice: ${choice}\n`);
    }
  } finally {
    rl.close();
  }
}

async function viewStatsInteractive(rl) {
  const period = (await ask(rl, 'Period? [r]eset (Sat 20h UTC) / [w]eek / [m]onth / [d] last N days / [h] last N hours / [D] from date / [a]ll (default: r): ')).trim().toLowerCase();
  const v = {};
  if (period === 'w') v.week = true;
  else if (period === 'm') v.month = true;
  else if (period === 'd') {
    const n = (await ask(rl, 'Days: ')).trim();
    v.last = `${parseInt(n, 10) || 7}d`;
  } else if (period === 'h') {
    const n = (await ask(rl, 'Hours: ')).trim();
    v.last = `${parseInt(n, 10) || 24}h`;
  } else if (period === 'D') {
    const from = (await ask(rl, 'From (YYYY-MM-DD): ')).trim();
    if (from) v.from = from;
  } else if (period === 'a') v.all = true;
  else v['since-reset'] = true;

  const group = (await ask(rl, 'Group by? [p]roject / [s]ession / [d]ay / [m]odel / [n]o grouping (default: auto): ')).trim().toLowerCase();
  if (group === 'p') v['by-project'] = true;
  else if (group === 's') v['by-session'] = true;
  else if (group === 'd') v['by-day'] = true;
  else if (group === 'm') v['by-model'] = true;

  const fmt = (await ask(rl, 'Format? [t]able / [j]son (default: t): ')).trim().toLowerCase();
  if (fmt === 'j') v.json = true;

  try {
    runStats(v);
  } catch (e) {
    process.stderr.write(`error: ${e.message}\n`);
  }

  const parts = [];
  for (const [k, val] of Object.entries(v)) {
    if (val === true) parts.push(`--${k}`);
    else if (Array.isArray(val)) for (const x of val) parts.push(`--${k}=${x}`);
    else parts.push(`--${k}=${val}`);
  }
  process.stdout.write(`\n« Equivalent CLI: node recap.js ${parts.join(' ')} »\n`);
}

async function main() {
  const argv = process.argv.slice(2);
  let parsed;
  try {
    parsed = parseFlags(argv);
  } catch (e) {
    process.stderr.write(`error: ${e.message}\n\n${HELP}\n`);
    process.exit(2);
  }
  const v = parsed.values;
  if (v.help) {
    process.stdout.write(HELP + '\n');
    return;
  }

  if (argv.length === 0 && process.stdin.isTTY && !v['no-interactive']) {
    await menuLoop();
    return;
  }

  try {
    runStats(v);
  } catch (e) {
    process.stderr.write(`error: ${e.message}\n`);
    process.exit(1);
  }
}

main();
