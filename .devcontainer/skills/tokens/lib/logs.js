// tokens skill — local log discovery and event walking.
// Reads from <project-root>/.claude/tokens/logs/YYYY-MM/*.jsonl.
// Session 2 scope: current project only. Session 4 extends to cross-container.

const fs = require('node:fs');
const path = require('node:path');

function findProjectRoot(startDir = process.cwd()) {
  let dir = path.resolve(startDir);
  while (true) {
    if (fs.existsSync(path.join(dir, '.claude', 'tokens'))) return dir;
    if (fs.existsSync(path.join(dir, '.git'))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir) return startDir;
    dir = parent;
  }
}

function readConfig(projectRoot) {
  const p = path.join(projectRoot, '.claude', 'tokens', 'config.json');
  if (!fs.existsSync(p)) return null;
  try {
    return JSON.parse(fs.readFileSync(p, 'utf8'));
  } catch (e) {
    process.stderr.write(`warn: config.json parse failed: ${e.message}\n`);
    return null;
  }
}

function monthDirInWindow(monthName, window) {
  const m = /^(\d{4})-(\d{2})$/.exec(monthName);
  if (!m) return false;
  const year = parseInt(m[1], 10);
  const month = parseInt(m[2], 10);
  const monthStart = Date.UTC(year, month - 1, 1);
  const monthEnd = Date.UTC(year, month, 1) - 1;
  return monthEnd >= window.startEpoch && monthStart <= window.endEpoch;
}

function* walkEvents(projectRoot, window) {
  const logsDir = path.join(projectRoot, '.claude', 'tokens', 'logs');
  if (!fs.existsSync(logsDir)) return;
  const months = fs.readdirSync(logsDir).sort();
  for (const month of months) {
    if (!monthDirInWindow(month, window)) continue;
    const monthPath = path.join(logsDir, month);
    let entries;
    try {
      entries = fs.readdirSync(monthPath);
    } catch {
      continue;
    }
    for (const file of entries) {
      if (!file.endsWith('.jsonl')) continue;
      const full = path.join(monthPath, file);
      let content;
      try {
        content = fs.readFileSync(full, 'utf8');
      } catch (e) {
        process.stderr.write(`warn: cannot read ${full}: ${e.message}\n`);
        continue;
      }
      const lines = content.split('\n');
      for (const line of lines) {
        if (!line.trim()) continue;
        let ev;
        try {
          ev = JSON.parse(line);
        } catch (e) {
          process.stderr.write(`warn: bad JSONL in ${full}: ${e.message}\n`);
          continue;
        }
        if (!ev.tokens || typeof ev.cost_usd !== 'number' || !ev.ts) continue;
        const ts = Date.parse(ev.ts);
        if (Number.isNaN(ts)) continue;
        if (ts < window.startEpoch || ts > window.endEpoch) continue;
        yield {
          ts,
          ts_iso: ev.ts,
          session: ev.session,
          model: ev.model || 'unknown',
          project_root: ev.project_root || projectRoot,
          tokens: {
            in: ev.tokens.in || 0,
            cache_read: ev.tokens.cache_read || 0,
            cache_create: ev.tokens.cache_create || 0,
            out: ev.tokens.out || 0,
          },
          cost_usd: ev.cost_usd || 0,
        };
      }
    }
  }
}

module.exports = { findProjectRoot, readConfig, walkEvents, monthDirInWindow };

if (require.main === module) {
  const assert = require('node:assert/strict');
  const w = { startEpoch: Date.UTC(2026, 6, 1), endEpoch: Date.UTC(2026, 6, 31) };
  assert.equal(monthDirInWindow('2026-07', w), true);
  assert.equal(monthDirInWindow('2026-06', w), false);
  assert.equal(monthDirInWindow('2026-08', w), false);
  assert.equal(monthDirInWindow('bad', w), false);
  const root = findProjectRoot('/workspace');
  assert.equal(root, '/workspace', `findProjectRoot(/workspace) = ${root}`);
  console.log('lib/logs.js: 5/5 ok');
}
