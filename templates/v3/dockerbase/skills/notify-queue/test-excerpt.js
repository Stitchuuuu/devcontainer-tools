#!/usr/bin/env node
// test-excerpt.js — assertions for hook.js excerpt + decode helpers.
//
// Run: `node .devcontainer/skills/notify-queue/test-excerpt.js`
// Exits 0 on success, throws + non-zero on failure.

const assert = require('assert')
const { excerptV1, excerptV2, decodeUnicodeEscapes, buildLine } = require('./hook')

// 1. The reported mojibake : `\uXXXX` literals in a Recap line.
const reported =
	'Some prior text.\n\n**Recap** — Sources cit\\u00e9es, ' +
	'niveau de confiance d\\u00e9clar\\u00e9'
const out = excerptV2(reported)
assert.ok(out.includes('citées'),  `expected "citées" — got: ${out}`)
assert.ok(out.includes('déclaré'), `expected "déclaré" — got: ${out}`)
assert.ok(!out.includes('\\u'),    `unexpected "\\u" — got: ${out}`)

// 2. Raw UTF-8 must pass through untouched.
const ok = '**Recap** — Tests passants, é and — preserved'
assert.strictEqual(excerptV2(ok), 'Tests passants, é and — preserved')

// 3. V1 fallback also decodes.
const v1in = 'First usable line with caf\\u00e9 inside'
assert.ok(excerptV1(v1in).includes('café'),
	`V1 expected "café" — got: ${excerptV1(v1in)}`)

// 4. Conservative : escaped backslash before `u00e9` is left alone.
//    JS literal `'\\\\u00e9'` is the 6-char string `\\u00e9`.
assert.strictEqual(decodeUnicodeEscapes('\\\\u00e9'), '\\\\u00e9')

// 5. Other backslash sequences are not touched.
assert.strictEqual(decodeUnicodeEscapes('line1\\nline2'), 'line1\\nline2')
assert.strictEqual(decodeUnicodeEscapes('a\\\\b'),        'a\\\\b')

// 6. Surrogate pair (emoji) decodes to the right codepoint.
assert.strictEqual(decodeUnicodeEscapes('\\uD83D\\uDE00'), '😀')

// --- buildLine — session 8 sender + notif_id fields ---
//
// The daemon reads `line.sender` and `line.notif_id` from every queue
// line and threads them into `notif send --sender X --id Y`. Missing
// either would silently make `notif remove` unable to dismiss the
// banner later, so both must land on every event class.

const SID = '11111111-2222-3333-4444-555555555555'

const stop = buildLine('stop', {
	session_id:            SID,
	last_assistant_message: '**Recap** — Ok',
})
assert.strictEqual(stop.sender, 'default', 'stop.sender defaults to "default"')
assert.match(stop.notif_id, /^stop-11111111-\d+$/,
	`stop.notif_id shape: ${stop.notif_id}`)

const notif = buildLine('notification', {
	session_id:        SID,
	notification_type: 'permission_prompt',
	message:           'hi',
})
assert.strictEqual(notif.sender, 'default')
assert.match(notif.notif_id, /^notification-11111111-\d+$/)

const perm = buildLine('permission_request', {
	session_id: SID,
	tool_name:  'Bash',
	tool_input: { command: 'ls' },
})
assert.strictEqual(perm.sender, 'default')
assert.match(perm.notif_id, /^permission_request-11111111-\d+$/)
assert.deepStrictEqual(perm.tool_input, { command: 'ls' }, 'tool_input passes through')

const replied = buildLine('user_replied', { session_id: SID })
assert.strictEqual(replied.sender, 'default')
assert.match(replied.notif_id, /^user_replied-11111111-\d+$/)

// notif_id changes across invocations even for the same session/event —
// Date.now() ticks at millisecond resolution and the two calls straddle
// at least one tick under any realistic test scheduler.
const a = buildLine('stop', { session_id: SID, last_assistant_message: 'x' })
// Busy-wait until the clock ticks so uniqueness holds even on machines
// with a coarse Date.now (some CI runners round to 4 ms).
const t0 = Date.now()
while (Date.now() === t0) { /* spin briefly */ }
const b = buildLine('stop', { session_id: SID, last_assistant_message: 'x' })
assert.notStrictEqual(a.notif_id, b.notif_id, 'notif_id must be unique per invocation')

console.log('test-excerpt.js — all assertions passed')
