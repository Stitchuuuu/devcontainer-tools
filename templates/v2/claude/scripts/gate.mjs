#!/usr/bin/env node
// The commit gate, read for me rather than parsed by hand every time.
//
// CLAUDE-project asks for two things before a commit is proposed, and both are
// easy to fudge : lint at 0/0/0 **including infos** (biome exits 0 with warnings,
// so the exit code proves nothing), and an explicit pass/fail/skip count with the
// diagnostic line of every skip quoted. This runs both and prints exactly that.
//
// Exit code is 1 unless lint is 0/0/0 and no test failed — so it can gate a
// chain, which `wtf lint ; wtf test` cannot.
//
//   gate.mjs                 lint + the whole suite
//   gate.mjs --components    lint + one layer
//   gate.mjs --lint-only

import { execFileSync } from 'node:child_process'

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

const run = args => {
	try {
		return JSON.parse(execFileSync('node', args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }))
	} catch (e) {
		// The aggregators exit non-zero on failures but still print the blob.
		const out = e.stdout?.toString() ?? ''
		const at = out.indexOf('{')
		if (at < 0) throw e
		return JSON.parse(out.slice(at))
	}
}

let ok = true

if (!argv.includes('--test-only')) {
	const lint = run(['scripts/lint-json.mjs'])
	const t = lint.totals
	const clean = !t.errors && !t.warnings && !t.infos
	ok &&= clean
	console.log(`LINT   ${t.errors} errors · ${t.warnings} warnings · ${t.infos} infos${clean ? '' : '   ← not clean'}`)
	for (const d of lint.biome?.diagnostics ?? []) {
		console.log(`  ${d.severity.padEnd(7)} ${d.location?.path ?? ''}  ${String(d.message).slice(0, 100)}`)
	}
	for (const d of lint.vueTsc ?? []) console.log(`  vue-tsc ${JSON.stringify(d).slice(0, 160)}`)
}

if (!argv.includes('--lint-only')) {
	const test = run(['scripts/run-tests-aggregated.mjs', '--json', ...flags, ...filters])
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
		const human = execFileSync('node', ['scripts/run-tests-aggregated.mjs', ...flags, ...filters], {
			encoding: 'utf8',
			maxBuffer: 64 * 1024 * 1024,
		})
		for (const line of human.split('\n')) if (/\(skipped\)/.test(line)) console.log(`  ${line.trim()}`)
	}
}

console.log(ok ? '\nGATE   clear' : '\nGATE   BLOCKED')
process.exit(ok ? 0 : 1)
