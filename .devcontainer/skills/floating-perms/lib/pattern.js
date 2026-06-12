// pattern.js — canonicalize (tool_name, tool_input) into a single
// permission-shaped string that compares equal to settings.local.json
// `permissions.allow` entries.
//
// Canonical forms:
//   Bash(<cmd>:*)
//   Edit(<dir>/**)
//   Write(<dir>/**)
//   Read(<dir>/**)
//
// Lossy by design. We don't try to subsume Bash(curl:-sL) under
// Bash(curl:*) via regex — too brittle. Equality on canonical strings is
// the contract for both the spike counter and the "already allowed" gate.

const path = require('path')

const BASH_STRIP_PREFIX = /^\s*(?:env\s+[A-Z_][A-Z0-9_]*=\S+\s+)+/i
const BASH_LEADING_CD   = /^\s*cd\s+\S+\s*&&\s*/

// Tools that take a {command} input (Bash) vs tools that take a {file_path}.
const FILE_TOOLS = new Set(['Edit', 'Write', 'Read', 'NotebookEdit'])

// All tools whose prompts we want to detect. Read is included — even
// when the tool name is in allow, the path it targets can still prompt
// (out-of-workspace paths). Same story for Bash: grep being allowed at
// the command level doesn't grant access to /tmp/scratch.
const SPIKE_TOOLS = new Set(['Bash', 'Edit', 'Write', 'Read', 'NotebookEdit'])

function canonicalizeBash(command) {
	if (typeof command !== 'string') return null
	let s = command.trim()
	// peel env VAR=x prefixes
	while (BASH_STRIP_PREFIX.test(s)) s = s.replace(BASH_STRIP_PREFIX, '')
	// peel leading `cd path && `
	while (BASH_LEADING_CD.test(s)) s = s.replace(BASH_LEADING_CD, '')
	// peel sudo
	if (/^sudo\s+/.test(s)) s = s.replace(/^sudo\s+/, '')
	const first = s.split(/\s+/)[0]
	if (!first) return null
	// strip a leading path prefix (./foo, /usr/bin/foo → foo)
	const cmd = path.basename(first)
	if (!cmd) return null
	return `Bash(${cmd}:*)`
}

// Bucket a path to its first 2 segments under root. Examples:
//   /workspace/src/foo/bar.ts        → /workspace/src/**
//   /tmp/scratch/file                → /tmp/scratch/**
//   /home/node/.config/something/x   → /home/node/.config/**
function canonicalizeDir(filePath) {
	if (typeof filePath !== 'string' || !filePath) return null
	const abs = path.resolve(filePath)
	const parts = abs.split('/').filter(Boolean)
	if (parts.length === 0) return '/**'
	const head = parts.slice(0, 2).join('/')
	return `/${head}/**`
}

// Extract the first out-of-workspace absolute path from a Bash command.
// Returns a canonicalized Read(<bucket>/**) pattern (the most permissive
// path-shape) or null if no such path is present. Used when the command
// itself is already in allow but the prompt comes from the path.
//
// Path detection: naive — matches /<seg>/<seg>... with at least two
// segments. Filters out /workspace/* (already allowed by default for
// the cwd).
//
// IMPORTANT: only applied to commands in PATH_POSITIONAL_BASH below. We
// can't safely scan arbitrary commands like `git`, `sh`, `node`, or any
// command that accepts a heredoc / multi-line message — example paths
// inside a commit message or a script body would yield false-positive
// fallbacks. Whitelist of tools whose first positional argument is
// reliably a real filesystem path that, if outside /workspace, would
// actually have prompted the user.
const ABS_PATH_RE = /(?<![\w\\\-])(\/[a-zA-Z0-9_.\-]+(?:\/[a-zA-Z0-9_.\-]+)+)/g

const PATH_POSITIONAL_BASH = new Set([
	'grep', 'rg', 'ag', 'ack',
	'ls', 'cat', 'head', 'tail', 'less', 'more', 'file', 'stat',
	'find', 'fd', 'locate',
	'sed', 'awk', 'cut', 'tr',
	'wc', 'sort', 'uniq', 'diff', 'cmp',
	'tar', 'zip', 'unzip', 'gzip', 'gunzip',
	'cp', 'mv', 'ln',
	'mkdir', 'touch', 'realpath', 'readlink', 'basename', 'dirname'
])

function extractOutOfWorkspacePath(command) {
	if (typeof command !== 'string') return null
	const firstToken = command.trim().split(/\s+/)[0]
	if (!firstToken) return null
	const cmdName = path.basename(firstToken)
	if (!PATH_POSITIONAL_BASH.has(cmdName)) return null
	const matches = command.match(ABS_PATH_RE)
	if (!matches) return null
	for (const p of matches) {
		if (p === '/workspace' || p.startsWith('/workspace/')) continue
		const dir = canonicalizeDir(p)
		if (dir) return `Read(${dir})`
	}
	return null
}

// Tolerant allow-list check. Bash patterns in settings.local.json appear
// in three historical shapes: `Bash(cmd:*)` (canonical Claude Code),
// `Bash(cmd*)` (no space, e.g. `Bash(git log*)`), and `Bash(cmd *)`
// (space-prefix, e.g. `Bash(ls *)`). Treat all three as equivalent —
// the user shouldn't have to know which shape they used.
function isAllowed(pattern, allowList) {
	if (!pattern || !Array.isArray(allowList)) return false
	if (allowList.includes(pattern)) return true
	const bash = pattern.match(/^Bash\(([^:]+):\*\)$/)
	if (bash) {
		const cmd = bash[1].trim()
		if (allowList.includes(`Bash(${cmd}*)`)) return true
		if (allowList.includes(`Bash(${cmd} *)`)) return true
	}
	return false
}

// Paths under /workspace are auto-allowed by Claude Code's default sandbox
// (cwd of the devcontainer). The hook still fires for them, but the user
// never actually sees a prompt, so they must not count toward spikes.
function isCwdPath(filePath) {
	if (typeof filePath !== 'string') return false
	const abs = path.resolve(filePath)
	return abs === '/workspace' || abs.startsWith('/workspace/')
}

function canonicalize(toolName, toolInput, allowList) {
	if (!toolName || !toolInput || typeof toolInput !== 'object') return null
	if (toolName === 'Bash') {
		const bash = canonicalizeBash(toolInput.command)
		if (!bash) return null
		if (allowList && isAllowed(bash, allowList)) {
			return extractOutOfWorkspacePath(toolInput.command)
		}
		return bash
	}
	if (FILE_TOOLS.has(toolName)) {
		const p = toolInput.file_path || toolInput.notebook_path
		if (isCwdPath(p)) return null
		const dir = canonicalizeDir(p)
		if (!dir) return null
		return `${toolName}(${dir})`
	}
	return null
}

function isSpikeTool(toolName) {
	return SPIKE_TOOLS.has(toolName)
}

module.exports = { canonicalize, isAllowed, isSpikeTool, extractOutOfWorkspacePath }
