// pattern.test.js — exercise canonicalize / canonicalizeBash /
// canonicalizeDir. Pure functions, no state needed.

const { test } = require('node:test')
const assert   = require('node:assert/strict')

const { canonicalize, canonicalizeBash, canonicalizeDir } =
	require('../lib/pattern')

test('canonicalize returns a string for Bash without an allow-list arg', () => {
	assert.equal(canonicalize('Bash', { command: 'foo --bar' }), 'Bash(foo:*)')
})

test('canonicalize returns null for unknown tools (meta / future)', () => {
	for (const t of ['ExitPlanMode', 'AskUserQuestion', 'TodoWrite', 'Task',
		'WebSearch', 'mcp__whatever__do_something']) {
		assert.equal(canonicalize(t, { foo: 'bar' }), null, `tool ${t}`)
	}
})

test('canonicalize returns null on missing/invalid input', () => {
	assert.equal(canonicalize(null,   { command: 'x' }), null)
	assert.equal(canonicalize('Bash', null),             null)
	assert.equal(canonicalize('Bash', 'not an object'),  null)
	assert.equal(canonicalize('Bash', { command: 42 }),  null)
})

test('canonicalizeBash strips a leading env VAR=x prefix', () => {
	// Note: only the canonical `env VAR=x cmd …` shape is stripped.
	// Multi-VAR forms (`env FOO=1 BAR=2 cmd`) are a known
	// pre-existing limitation of the regex.
	assert.equal(
		canonicalizeBash('env FOO=1 curl https://x'),
		'Bash(curl:*)'
	)
})

test('canonicalizeBash strips leading `cd path && `', () => {
	assert.equal(
		canonicalizeBash('cd /tmp && grep foo bar'),
		'Bash(grep:*)'
	)
})

test('canonicalizeBash strips sudo', () => {
	assert.equal(canonicalizeBash('sudo apt-get install -y curl'),
		'Bash(apt-get:*)')
})

test('canonicalizeBash strips a leading directory prefix on the command', () => {
	assert.equal(canonicalizeBash('/usr/bin/python3 -m foo'),
		'Bash(python3:*)')
	assert.equal(canonicalizeBash('./scripts/run.sh'),
		'Bash(run.sh:*)')
})

test('canonicalizeBash returns null on empty / non-string input', () => {
	assert.equal(canonicalizeBash(''),   null)
	assert.equal(canonicalizeBash(null), null)
	assert.equal(canonicalizeBash(42),   null)
})

test('canonicalizeBash returns non-null on shell-form inputs (best-effort)', () => {
	// Shell-form: `if [ ... ]; then ...`, `TOK=$(...)`, etc. They never
	// prompt in practice (because Claude Code resolves them through Bash
	// and the inner command may or may not be in allow), but the
	// canonicalizer is best-effort labelling — it must return *something*
	// stable rather than null.
	assert.ok(canonicalizeBash('if [ -f /tmp/x ]; then echo ok; fi'))
	assert.ok(canonicalizeBash('TOK=$(jq .foo file)'))
	assert.ok(canonicalizeBash('grep foo /dev/null'))
})

test('canonicalizeDir buckets at depth 2 from root', () => {
	// Depth 2 = first two path segments under root, deeper paths
	// collapse to the same bucket.
	assert.equal(canonicalizeDir('/workspace/src/foo/bar.ts'),
		'/workspace/src/**')
	assert.equal(canonicalizeDir('/tmp/scratch/file'),
		'/tmp/scratch/**')
	assert.equal(canonicalizeDir('/home/node/.config/something/x'),
		'/home/node/**')
})

test('canonicalizeDir returns null on `/`', () => {
	// Edge case: input that resolves to `/` should NOT bucket to the
	// hyper-permissive `/**`. Returning null lets the observer skip
	// the entry entirely.
	assert.equal(canonicalizeDir('/'), null)
})

test('canonicalizeDir normalises double-slash inputs', () => {
	// Audit log entries like `Read(//home/node/...)` used to mis-bucket.
	// Both shapes should bucket identically.
	assert.equal(
		canonicalizeDir('//home/node/x'),
		canonicalizeDir('/home/node/x')
	)
})

test('canonicalizeDir returns null on missing / non-string input', () => {
	assert.equal(canonicalizeDir(''),        null)
	assert.equal(canonicalizeDir(undefined), null)
	assert.equal(canonicalizeDir(42),        null)
})

test('canonicalize wraps file tools with the tool name', () => {
	assert.equal(
		canonicalize('Edit',  { file_path: '/tmp/scratch/x.txt' }),
		'Edit(/tmp/scratch/**)'
	)
	assert.equal(
		canonicalize('Write', { file_path: '/home/node/.config/foo' }),
		'Write(/home/node/**)'
	)
	assert.equal(
		canonicalize('Read',  { file_path: '/workspace/src/x.ts' }),
		'Read(/workspace/src/**)'
	)
	assert.equal(
		canonicalize('NotebookEdit', { notebook_path: '/tmp/scratch/nb.ipynb' }),
		'NotebookEdit(/tmp/scratch/**)'
	)
})
