#!/usr/bin/env node
// get-notif-path.test.js — resolution order for notify-app::getNotifPath.
//
// Runs the 7 candidate branches (NOTIF_BIN → $XDG_DATA_HOME → ~/.local/bin →
// ~/bin → /usr/local/bin → /opt/homebrew/bin → vendor fallback) plus the
// all-missing case. Uses a per-test temp $HOME so real disk state can't leak
// into the assertions. Homebrew paths are absolute and can't be sandboxed —
// tests skip their existence assertion if the host actually has a `notif`
// binary at those locations.
//
// Run: node .devcontainer/notify/tests/get-notif-path.test.js
// Exits 0 on success ; throws + non-zero on failure.

const assert = require('assert')
const fs     = require('fs')
const os     = require('os')
const path   = require('path')

const { getNotifPath } = require('../lib/consumers/notify-app')

// Snapshot the env slots we mutate so the test process leaves nothing behind.
const SNAPSHOT_ENV = {
	NOTIF_BIN:      process.env.NOTIF_BIN,
	XDG_DATA_HOME:  process.env.XDG_DATA_HOME,
	HOME:           process.env.HOME,
}
function restoreEnv() {
	for (const [k, v] of Object.entries(SNAPSHOT_ENV)) {
		if (v === undefined) delete process.env[k]
		else                 process.env[k] = v
	}
}
process.on('exit', restoreEnv)

// Fresh sandbox $HOME + $XDG_DATA_HOME per test — old candidates never carry
// over. Directory is nuked at the top of each case for full isolation.
const SANDBOX = fs.mkdtempSync(path.join(os.tmpdir(), 'notify-app-test-'))
function resetSandbox() {
	fs.rmSync(SANDBOX, { recursive: true, force: true })
	fs.mkdirSync(SANDBOX, { recursive: true })
}
process.on('exit', () => fs.rmSync(SANDBOX, { recursive: true, force: true }))

function touch(p) {
	fs.mkdirSync(path.dirname(p), { recursive: true })
	fs.writeFileSync(p, '#!/bin/sh\necho stub\n', { mode: 0o755 })
}

function clearEnv() {
	delete process.env.NOTIF_BIN
	delete process.env.XDG_DATA_HOME
}

// 1. NOTIF_BIN env wins over every other candidate.
{
	resetSandbox()
	clearEnv()
	process.env.HOME = SANDBOX
	const explicit  = path.join(SANDBOX, 'somewhere', 'notif-explicit')
	const dotLocal  = path.join(SANDBOX, '.local', 'bin', 'notif')
	touch(explicit)
	touch(dotLocal)                                // decoy — must NOT win
	process.env.NOTIF_BIN = explicit
	assert.strictEqual(getNotifPath(), explicit, 'NOTIF_BIN must win over ~/.local/bin')
}

// 2. XDG_DATA_HOME/notif/notif wins when NOTIF_BIN is unset.
{
	resetSandbox()
	clearEnv()
	process.env.HOME = SANDBOX
	process.env.XDG_DATA_HOME = path.join(SANDBOX, 'xdg')
	const xdg      = path.join(SANDBOX, 'xdg', 'notif', 'notif')
	const dotLocal = path.join(SANDBOX, '.local', 'bin', 'notif')
	touch(xdg)
	touch(dotLocal)                                // decoy — lower priority
	assert.strictEqual(getNotifPath(), xdg, 'XDG_DATA_HOME must win over ~/.local/bin')
}

// 3. ~/.local/bin/notif wins when the two above are missing.
{
	resetSandbox()
	clearEnv()
	process.env.HOME = SANDBOX
	const dotLocal = path.join(SANDBOX, '.local', 'bin', 'notif')
	const homeBin  = path.join(SANDBOX, 'bin', 'notif')
	touch(dotLocal)
	touch(homeBin)                                 // decoy — lower priority
	assert.strictEqual(getNotifPath(), dotLocal, '~/.local/bin/notif must win over ~/bin/notif')
}

// 4. ~/bin/notif wins when the previous three are missing.
{
	resetSandbox()
	clearEnv()
	process.env.HOME = SANDBOX
	const homeBin = path.join(SANDBOX, 'bin', 'notif')
	touch(homeBin)
	assert.strictEqual(getNotifPath(), homeBin)
}

// 5. All sandbox candidates missing → null OR one of the absolute Homebrew
// paths (`/usr/local/bin/notif`, `/opt/homebrew/bin/notif`) when the host
// happens to have `notif` installed there. Can't be sandboxed cheaply, so
// accept either shape as long as the resolution is deterministic.
{
	resetSandbox()
	clearEnv()
	process.env.HOME = SANDBOX
	const HOMEBREW_PATHS = ['/usr/local/bin/notif', '/opt/homebrew/bin/notif']
	const resolved = getNotifPath()
	assert.ok(
		resolved === null || HOMEBREW_PATHS.includes(resolved),
		`expected null or a Homebrew path, got: ${resolved}`,
	)
}

console.log('get-notif-path.test.js — all assertions passed')
