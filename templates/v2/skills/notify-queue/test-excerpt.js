#!/usr/bin/env node
// test-excerpt.js — assertions for hook.js excerpt + decode helpers.
//
// Run: `node .devcontainer/skills/notify-queue/test-excerpt.js`
// Exits 0 on success, throws + non-zero on failure.

const assert = require('assert')
const { excerptV1, excerptV2, decodeUnicodeEscapes } = require('./hook')

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

console.log('test-excerpt.js — all assertions passed')
