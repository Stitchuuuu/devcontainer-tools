// pattern.js — canonicalize (tool_name, tool_input) into a single
// permission-shaped string for the audit log and the deny message.
//
// Canonical forms:
//   Bash(<cmd>:*)
//   Edit(<dir>/**)
//   Write(<dir>/**)
//   Read(<dir>/**)
//   NotebookEdit(<dir>/**)
//
// Lossy by design. The result is best-effort labelling — it's what
// the user will see in the deny reason and the audit log, so it has
// to be human-readable, but it isn't load-bearing for any equality
// check (the observational spike detector only counts entries, it
// doesn't match patterns against an allow list).

const path = require('path')

const BASH_STRIP_PREFIX = /^\s*(?:env\s+[A-Z_][A-Z0-9_]*=\S+\s+)+/i
const BASH_LEADING_CD   = /^\s*cd\s+\S+\s*&&\s*/

const FILE_TOOLS = new Set(['Edit', 'Write', 'Read', 'NotebookEdit'])

function canonicalizeBash(command) {
	if (typeof command !== 'string') return null
	let s = command.trim()
	while (BASH_STRIP_PREFIX.test(s)) s = s.replace(BASH_STRIP_PREFIX, '')
	while (BASH_LEADING_CD.test(s)) s = s.replace(BASH_LEADING_CD, '')
	if (/^sudo\s+/.test(s)) s = s.replace(/^sudo\s+/, '')
	const first = s.split(/\s+/)[0]
	if (!first) return null
	const cmd = path.basename(first)
	if (!cmd) return null
	return `Bash(${cmd}:*)`
}

// file_path inputs are always concrete files (Edit/Write/Read/NotebookEdit
// never target directories), so the canonical bucket is the parent dir.
// Examples:
//   /tmp/xxx                         → /tmp/**
//   /tmp/extensions.js               → /tmp/**
//   /tmp/scratch/file.txt            → /tmp/scratch/**
//   /workspace/src/foo/bar.ts        → /workspace/src/foo/**
//   /home/node/.config/something/x   → /home/node/.config/something/**
// Edge cases:
//   '/'                              → null (no parent)
//   '/foo'                           → null (parent is `/`, too broad)
//   '//a/b/c'                        → /a/b/** (double-slash normalised)
function canonicalizeDir(filePath) {
	if (typeof filePath !== 'string' || !filePath) return null
	const abs = path.resolve(filePath).replace(/\/+/g, '/')
	const dir = path.dirname(abs)
	if (!dir || dir === '/' || dir === '.') return null
	return `${dir}/**`
}

function canonicalize(toolName, toolInput) {
	if (!toolName || !toolInput || typeof toolInput !== 'object') return null
	if (toolName === 'Bash') {
		return canonicalizeBash(toolInput.command)
	}
	if (FILE_TOOLS.has(toolName)) {
		const p = toolInput.file_path || toolInput.notebook_path
		const dir = canonicalizeDir(p)
		if (!dir) return null
		return `${toolName}(${dir})`
	}
	return null
}

module.exports = { canonicalize, canonicalizeBash, canonicalizeDir }
