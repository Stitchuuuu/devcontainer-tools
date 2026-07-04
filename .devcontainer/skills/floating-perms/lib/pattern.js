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

const FILE_TOOLS = new Set(['Edit', 'Write', 'Read', 'NotebookEdit'])

// Tokenize the first command of a bash string. Respects single/double
// quotes and backslash escapes. Stops at |, &, ;, <, > (hard operators
// or redirection starts). Surfaces && as its own token so callers can
// step past `cd X && cmd` chains. Parens deliberately stay inside
// tokens — treating them as boundaries would break command-substitution
// shapes like `TOK=$(jq .foo file)` whose best-effort label depends on
// the assign-skip filter finding a non-assign token after `TOK=$(jq`.
function tokenizeFirstCommand(s) {
	const tokens = []
	let cur = ''
	let inSingle = false, inDouble = false
	const push = () => { if (cur !== '') { tokens.push(cur); cur = '' } }

	for (let i = 0; i < s.length; i++) {
		const c = s[i]
		if (inSingle) {
			if (c === "'") inSingle = false
			else cur += c
			continue
		}
		if (inDouble) {
			if (c === '\\' && i + 1 < s.length && '"\\$`'.includes(s[i + 1])) {
				cur += s[++i]
			} else if (c === '"') inDouble = false
			else cur += c
			continue
		}
		if (c === "'")  { inSingle = true; continue }
		if (c === '"')  { inDouble = true; continue }
		if (c === '\\' && i + 1 < s.length) { cur += s[++i]; continue }
		if (/\s/.test(c)) { push(); continue }
		if (c === '&' && s[i + 1] === '&') { push(); tokens.push('&&'); i++; continue }
		if ('|&;<>'.includes(c)) { push(); return tokens }
		cur += c
	}
	push()
	return tokens
}

// Well-known command wrappers that must be skipped to reach the real
// command root. Kept small on purpose — only shapes that actually
// appear in Claude Code Bash inputs.
const PREFIX_CMDS = new Set(['sudo', 'env', 'nice', 'nohup', 'exec', 'command', 'time'])
const ASSIGN_RE   = /^[A-Za-z_][A-Za-z0-9_]*=/

function canonicalizeBash(command) {
	if (typeof command !== 'string') return null
	const tokens = tokenizeFirstCommand(command.trim())
	let i = 0
	// Step past `cd X && …` chains (may repeat).
	while (tokens[i] === 'cd' && tokens[i + 2] === '&&') i += 3
	// Skip wrappers (sudo/env/…) and bare VAR=value assignments.
	while (i < tokens.length && (PREFIX_CMDS.has(tokens[i]) || ASSIGN_RE.test(tokens[i]))) i++
	const first = tokens[i]
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
