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

test('canonicalizeBash: bare RUSTFLAGS= assignment with quoted value', () => {
	// Prod regression from 2026-07-04: naive whitespace split extracted
	// `macos-sdk` from the quoted value's path component.
	assert.equal(canonicalizeBash(
		'RUSTFLAGS="-C link-arg=-F/workspace/apps/notifier/vendor/macos-sdk/System/Library/Frameworks" cargo zigbuild --release'
	), 'Bash(cargo:*)')
})

test('canonicalizeBash: env + quoted value with spaces + redirections', () => {
	// Prod regression: old regex stripped `env RUSTFLAGS="-C ` (stopping
	// at the space inside quotes) and returned `Bash(Frameworks":*)`.
	assert.equal(canonicalizeBash(
		'env RUSTFLAGS="-C link-arg=-F/workspace/apps/notifier/vendor/macos-sdk/System/Library/Frameworks" cargo zigbuild --release 2>&1 > /tmp/build.log'
	), 'Bash(cargo:*)')
})

test('canonicalizeBash: multi-var env prefix', () => {
	assert.equal(canonicalizeBash('env FOO=1 BAR=2 cmd arg'), 'Bash(cmd:*)')
})

test('canonicalizeBash: pipe stops at the first command', () => {
	assert.equal(canonicalizeBash('cargo build 2>&1 | tail -25'), 'Bash(cargo:*)')
})

test('canonicalizeBash: quoted args do not confuse the first-token pick', () => {
	assert.equal(canonicalizeBash('git commit -m "some message with dashes"'),
		'Bash(git:*)')
})

test('canonicalizeBash: single-quoted assignment value', () => {
	assert.equal(canonicalizeBash("FOO='single quoted' cmd"), 'Bash(cmd:*)')
})

test('canonicalizeBash: backslash-escaped space joins tokens', () => {
	assert.equal(canonicalizeBash('/usr/bin/my\\ cmd'), 'Bash(my cmd:*)')
})

test('canonicalizeBash: cd with quoted path still strips', () => {
	assert.equal(canonicalizeBash('cd "/tmp/a b" && grep foo bar'), 'Bash(grep:*)')
})

test('canonicalizeDir buckets to the parent dir of a file path', () => {
	// file_path inputs are always files (Edit/Write/Read/NotebookEdit
	// never target directories), so the canonical bucket is the parent.
	assert.equal(canonicalizeDir('/tmp/xxx'),
		'/tmp/**')
	assert.equal(canonicalizeDir('/tmp/extensions.js'),
		'/tmp/**')
	assert.equal(canonicalizeDir('/tmp/scratch/file'),
		'/tmp/scratch/**')
	assert.equal(canonicalizeDir('/workspace/src/foo/bar.ts'),
		'/workspace/src/foo/**')
	assert.equal(canonicalizeDir('/home/node/.config/something/x'),
		'/home/node/.config/something/**')
})

test('canonicalizeDir returns null on `/` and on `/foo` (parent too broad)', () => {
	// Edge cases: parent resolving to `/` would bucket to the
	// hyper-permissive `/**`. Returning null lets the observer skip
	// the entry entirely.
	assert.equal(canonicalizeDir('/'),    null)
	assert.equal(canonicalizeDir('/foo'), null)
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
		'Write(/home/node/.config/**)'
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
