#!/usr/bin/env node
// The commit gate, read for me rather than parsed by hand every time.
//
// CLAUDE-project asks for two things before a commit is proposed, and both are
// easy to fudge : lint at 0/0/0 **including infos** (biome exits 0 with warnings,
// so the exit code proves nothing), and an explicit pass/fail/skip count with the
// diagnostic line of every skip quoted. This runs both and prints exactly that.
//
// ─── THE CONTRACT ────────────────────────────────────────────────────────────
// This file is a READER. It cannot lint and cannot test ; it runs whatever the
// project declares and renders the result. A project wires each half in one of
// two ways, tried in this order :
//
//   1. an aggregator script — scripts/lint-json.mjs, scripts/run-tests-aggregated.mjs
//   2. a declared wtf command — `wtf lint`, `wtf test`
//
// For a FULL-strength verdict the chosen source must print JSON on stdout :
//
//   lint  { "totals": { "errors": N, "warnings": N, "infos": N },
//           "biome":  { "diagnostics": [ { severity, location: { path }, message } ] },
//           "vueTsc": [ … ] }                                    ← both optional
//   test  { "totals":   { "pass": N, "fail": N, "skip": N },
//           "layers":   { "<key>": { short, title, pass, fail, skip } },
//           "failures": [ { file, name, message } ] }            ← both optional
//
// Most tools emit that with one flag — biome `--reporter=json`, eslint `-f json`,
// vitest `--reporter=json` — so the adapter is usually a shell pipeline, not a
// script.
//
// ─── WHY IT REFUSES TO SAY "clear" WHEN IT MEASURED NOTHING ──────────────────
// A half it could not measure is reported as PARTIAL, exit 2 — never as a pass.
// Same three tiers, same reason, as `wtf image release-check` : a bench that
// cannot run must not read like a bench that passed.
//
//   exit 0   GATE clear     both halves measured, nothing failed
//   exit 1   GATE BLOCKED   something failed
//   exit 2   GATE PARTIAL   nothing failed, but a half was not measured
//
//   gate.mjs                 lint + the whole suite
//   gate.mjs --components    lint + one layer
//   gate.mjs --lint-only

import { existsSync } from 'node:fs'
import { spawnSync } from 'node:child_process'

const HELP = `gate.mjs [options]

  --lint-only            skip the suite
  --test-only            skip lint
  --node --vue --unit --integration --remote --composables --components
                         narrow the suite, same flags as \`wtf test\`
  <filter> …             substring filters on test file paths
`

const argv = process.argv.slice(2)
if (argv.includes('--help')) {
	process.stdout.write(HELP)
	process.exit(0)
}

const LAYERS = ['node', 'vue', 'unit', 'integration', 'remote', 'composables', 'components']
const flags = argv.filter(a => LAYERS.includes(a.replace(/^--/, '')) && a.startsWith('--'))
const filters = argv.filter(a => !a.startsWith('--'))

// Never throws, merges nothing, reports what happened. `missing` means the
// binary itself is absent (ENOENT) — distinct from "ran and failed".
const capture = (cmd, args) => {
	const r = spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 })
	if (r.error) return { missing: true, code: null, out: '', err: String(r.error.message) }
	return { missing: false, code: r.status, out: r.stdout ?? '', err: r.stderr ?? '' }
}

// The aggregators exit non-zero on failures but still print the blob, so parse
// the output rather than trust the status.
const parseJson = text => {
	const at = text.indexOf('{')
	if (at < 0) return null
	try {
		return JSON.parse(text.slice(at))
	} catch {
		return null
	}
}

// `wtf` prints "command not found" and still EXITS 0, so the status cannot be
// used to detect an undeclared command — measured, not assumed.
const WTF_UNDECLARED = /command not found/i

// Resolves one half to { state, data, code, how }.
//   full      — a source ran and emitted the documented JSON
//   exit-only — a source ran, no parseable JSON : verdict from the exit code,
//               which cannot see warnings
//   absent    — nothing declared
const resolve = (aggregator, wtfName, extraArgs, valid) => {
	if (existsSync(aggregator)) {
		const r = capture('node', [aggregator, ...extraArgs])
		const data = parseJson(r.out || r.err)
		if (data && valid(data)) return { state: 'full', data, code: r.code, how: `node ${aggregator}` }
		return { state: 'exit-only', data: null, code: r.code, how: `node ${aggregator}` }
	}

	const w = capture('wtf', [wtfName, ...extraArgs])
	if (w.missing) return { state: 'absent', reason: 'wtf is not installed, and no aggregator script exists' }
	if (WTF_UNDECLARED.test(w.out + w.err)) return { state: 'absent', reason: `no \`${wtfName}\` command is declared in .wtfcmd.yaml` }

	const data = parseJson(w.out)
	if (data && valid(data)) return { state: 'full', data, code: w.code, how: `wtf ${wtfName}` }
	return { state: 'exit-only', data: null, code: w.code, how: `wtf ${wtfName}` }
}

const SETUP = {
	lint: {
		aggregator: 'scripts/lint-json.mjs',
		shape: '{ "totals": { "errors": N, "warnings": N, "infos": N } }',
		blind: 'warnings and infos (biome and eslint both exit 0 with warnings)',
		flag: 'biome check --reporter=json .   ·   eslint -f json .',
	},
	test: {
		aggregator: 'scripts/run-tests-aggregated.mjs',
		shape: '{ "totals": { "pass": N, "fail": N, "skip": N } }',
		blind: 'the pass/skip split (a suite that skips everything still exits 0)',
		flag: 'vitest run --reporter=json   ·   jest --json',
	},
}

const notes = []
let ok = true
let measured = 0
let wanted = 0

const report = (name, r) => {
	const s = SETUP[name]
	if (r.state === 'absent') {
		notes.push({ name, kind: 'absent', reason: r.reason })
		console.log(`${name.toUpperCase().padEnd(6)} not configured`)
		return
	}
	if (r.state === 'exit-only') {
		notes.push({ name, kind: 'exit-only', how: r.how })
		ok &&= r.code === 0
		console.log(`${name.toUpperCase().padEnd(6)} exit ${r.code} via ${r.how} — blind to ${s.blind}`)
		return
	}
	measured++
	return r.data
}

if (!argv.includes('--test-only')) {
	wanted++
	const r = resolve(SETUP.lint.aggregator, 'lint', [], d => typeof d?.totals?.errors === 'number')
	const lint = report('lint', r)
	if (lint) {
		const t = lint.totals
		const clean = !t.errors && !t.warnings && !t.infos
		ok &&= clean
		console.log(`LINT   ${t.errors} errors · ${t.warnings} warnings · ${t.infos} infos${clean ? '' : '   ← not clean'}`)
		for (const d of lint.biome?.diagnostics ?? []) {
			console.log(`  ${d.severity.padEnd(7)} ${d.location?.path ?? ''}  ${String(d.message).slice(0, 100)}`)
		}
		for (const d of lint.vueTsc ?? []) console.log(`  vue-tsc ${JSON.stringify(d).slice(0, 160)}`)
	}
}

if (!argv.includes('--lint-only')) {
	wanted++
	const r = resolve(SETUP.test.aggregator, 'test', ['--json', ...flags, ...filters], d => typeof d?.totals?.pass === 'number')
	const test = report('test', r)
	if (test) {
		const t = test.totals
		ok &&= !t.fail
		console.log(`TEST   ${t.pass} pass · ${t.fail} fail · ${t.skip} skip`)
		for (const k in test.layers) {
			const l = test.layers[k]
			if (l.pass || l.fail || l.skip) console.log(`  ${l.short.padEnd(4)} ${String(l.pass).padStart(3)} / ${l.fail} / ${l.skip}   ${l.title}`)
		}
		for (const f of test.failures ?? []) console.log(`  FAIL ${f.file}\n       ${f.name}\n       ${String(f.message).slice(0, 200)}`)
		// A skip is not a pass. The gate asks for its reason quoted, so surface it here
		// rather than make the reader re-run in human format to find out.
		if (t.skip) {
			console.log(`\n  ${t.skip} skipped — reason, from the human format :`)
			const human = capture('node', [SETUP.test.aggregator, ...flags, ...filters]).out
			// The aggregator renders a skipped layer as `L4 · Remote — skipped — <why>`,
			// never `(skipped)` — the old pattern matched nothing and the gate printed
			// the "reason :" header with no reason under it. The second alternative
			// keeps per-test `(skipped)` markers working if a layer ever emits them.
			for (const line of human.split('\n')) {
				if (/skipped\s*[—-]|\(skipped\)/.test(line)) console.log(`  ${line.trim()}`)
			}
		}
	}
}

// ─── Verdict ─────────────────────────────────────────────────────────────────
if (!ok) {
	console.log('\nGATE   BLOCKED')
	process.exit(1)
}
if (measured === wanted) {
	console.log('\nGATE   clear')
	process.exit(0)
}

console.log(`\nGATE   PARTIAL — nothing failed, but ${wanted - measured} of ${wanted} halves were not measured at full strength`)
console.log('       This is NOT a pass. Do not propose a commit on it without saying so.')
console.log('\n─── SETUP NEEDED — what to ask the human ───────────────────────────')
for (const n of notes) {
	const s = SETUP[n.name]
	console.log(`\n  ${n.name}`)
	if (n.kind === 'absent') {
		console.log(`    ${n.reason}.`)
		console.log(`    Wire it either way :`)
		console.log(`      · declare a \`${n.name}\` command in .wtfcmd.yaml`)
		console.log(`      · or add ${s.aggregator}`)
	} else {
		console.log(`    ${n.how} ran but printed no parseable JSON, so the verdict`)
		console.log(`    came from its exit code alone — blind to ${s.blind}.`)
	}
	console.log(`    For a full-strength verdict its stdout must be :`)
	console.log(`      ${s.shape}`)
	console.log(`    Usually one flag away :  ${s.flag}`)
}
console.log(`
  ASK THE HUMAN : which linter and test runner this project uses, and whether
  to wire them as wtf commands or as the two aggregator scripts. Do not guess —
  a wrong adapter reports green having measured nothing, which is worse than
  this PARTIAL.

  Note: paths are resolved from the CURRENT DIRECTORY, so run this from the
  project root.`)
process.exit(2)
