#!/usr/bin/env node
// notify-queue/hook.js — single dispatcher for the 4 hook events
// (Stop, Notification, PermissionRequest, UserPromptSubmit).
//
// Reads the Claude Code hook JSON payload on stdin, writes one
// best-effort JSONL line to .devcontainer/notify-queue/<sid>.jsonl
// for the host-side daemon (session 2+) to consume.
//
// Always exits 0 with empty stdout — observational only, never
// blocks or alters Claude behavior.

const fs = require('fs')
const path = require('path')

const QUEUE_DIR = '/workspace/.devcontainer/notify/queue'
const MAX_EXCERPT = 200

const ARG_TO_EVENT = {
	stop: 'stop',
	notification: 'notification',
	permission_request: 'permission_request',
	user_prompt_submit: 'user_replied',
	// PreToolUse fires the instant the tool STARTS — i.e. right after the
	// user clicked Allow on the permission dialog. Best signal to cancel
	// a pending permission timer (PostToolUse arrives later, after the
	// tool also finished — useless for slow tools).
	pre_tool_use:  'tool_started',
	// PostToolUse fires when the tool COMPLETES. Kept as a secondary
	// cancel signal in case PreToolUse is skipped for some reason.
	post_tool_use: 'tool_finished'
}

function truncate(str, max) {
	if (typeof str !== 'string') return ''
	if (str.length <= max) return str
	return str.slice(0, max - 1) + '…'
}

// V2 : look for an explicit `**Recap** — <summary>` line near the end
// of the reply. Convention documented in CLAUDE-dev.md §14. Accepts
// em-dash (—), en-dash (–), or hyphen (-) as the separator. If the
// recap is present, that's the authoritative notif body. Otherwise
// fall back to the V1 heuristic.
function excerptV2(msg) {
	if (typeof msg !== 'string' || !msg) return ''
	const m = msg.match(/\*\*Recap\*\*\s*[—–-]\s*(.+?)\s*$/im)
	if (m && m[1]) return truncate(m[1].trim(), MAX_EXCERPT)
	return excerptV1(msg)
}

// V1 heuristic: extract a short, readable excerpt from a Claude
// markdown reply. Skip headers, code fences, tables, HTML comments.
// Strip the first usable line's basic markdown syntax.
function excerptV1(msg) {
	if (typeof msg !== 'string' || !msg) return ''
	const lines = msg.split('\n')
	let inFence = false
	for (let raw of lines) {
		const line = raw.trim()
		if (!line) continue
		if (line.startsWith('```')) { inFence = !inFence; continue }
		if (inFence) continue
		if (line.startsWith('#')) continue
		if (line.startsWith('|')) continue
		if (line.startsWith('<!--')) continue
		// strip markdown: links, bold, italic, inline code
		let clean = line
			.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
			.replace(/\*\*([^*]+)\*\*/g, '$1')
			.replace(/\*([^*]+)\*/g, '$1')
			.replace(/`([^`]+)`/g, '$1')
			.trim()
		if (clean) return truncate(clean, MAX_EXCERPT)
	}
	return ''
}

function readStdin() {
	try {
		const buf = fs.readFileSync(0, 'utf8')
		return JSON.parse(buf)
	} catch {
		return null
	}
}

// Read Claude Code's session name from the transcript JSONL at
// `transcript_path` (one of the hook payload fields).
//
// Three candidates, in priority order (highest wins) :
//   1. `"customTitle": "DOING - ..."` — the user-edited title (set via
//      the Claude Code VS Code extension's tab rename). Most authoritative.
//   2. `"aiTitle": "Build macOS host notification daemon"` — auto-generated
//      by Claude at session start and refined after analysis turns.
//   3. `"slug": "session-1-shimmying-manatee"` — friendly auto-stub
//      present on every transcript line.
//
// Scan surface : we read the TAIL (256 KB) for customTitle (always
// re-emitted on every subsequent line once set) + recent aiTitle, AND
// the HEAD (64 KB) as a fallback for aiTitle — Claude Code writes
// aiTitle sparsely (sometimes only a handful of times per session,
// with the first occurrence typically within the first ~30 KB). A
// tail-only scan misses sessions where the transcript grew past
// 256 KB without rewriting aiTitle, falling back to slug incorrectly.
const TRANSCRIPT_TAIL_BYTES = 256 * 1024
const TRANSCRIPT_HEAD_BYTES =  64 * 1024

function readSessionName(transcriptPath) {
	if (!transcriptPath || typeof transcriptPath !== 'string') return ''
	let fd
	try {
		const stat = fs.statSync(transcriptPath)
		if (stat.size <= 0) return ''
		fd = fs.openSync(transcriptPath, 'r')

		const tailSize  = Math.min(stat.size, TRANSCRIPT_TAIL_BYTES)
		const tailStart = Math.max(0, stat.size - tailSize)
		const tailBuf   = Buffer.alloc(tailSize)
		fs.readSync(fd, tailBuf, 0, tailSize, tailStart)
		const tail = tailBuf.toString('utf8')

		// 1. customTitle — always re-emitted on every subsequent line.
		const custom = lastMatch(tail, /"customTitle"\s*:\s*"([^"]+)"/g)
		if (custom) return custom

		// 2. aiTitle — try tail, then head if missed (sparse writes).
		let ai = lastMatch(tail, /"aiTitle"\s*:\s*"([^"]+)"/g)
		if (!ai && tailStart > 0) {
			const headSize = Math.min(tailStart, TRANSCRIPT_HEAD_BYTES)
			const headBuf  = Buffer.alloc(headSize)
			fs.readSync(fd, headBuf, 0, headSize, 0)
			ai = lastMatch(headBuf.toString('utf8'),
				/"aiTitle"\s*:\s*"([^"]+)"/g)
		}
		if (ai) return ai

		// 3. Slug — legit fallback only before the first aiTitle is written.
		return lastMatch(tail, /"slug"\s*:\s*"([^"]+)"/g) || ''
	} catch (_) { return '' }
	finally { if (fd !== undefined) try { fs.closeSync(fd) } catch (_) {} }
}

// Last capture group of a global regex, or '' if none matched.
function lastMatch(text, re) {
	let last = ''
	for (const m of text.matchAll(re)) if (m[1]) last = m[1]
	return last
}

function buildLine(eventName, payload) {
	const sid = payload.session_id
	if (!sid) return null

	const line = {
		ts: new Date().toISOString(),
		sid,
		event: eventName
	}

	// Attach Claude Code's session name (aiTitle from the transcript,
	// falls back to the auto-generated slug). Absent when no transcript.
	const name = readSessionName(payload.transcript_path)
	if (name) line.session_name = name

	if (eventName === 'stop') {
		line.last_message_excerpt = excerptV2(payload.last_assistant_message)
	} else if (eventName === 'notification') {
		line.notification_type = payload.notification_type || ''
		line.message = truncate(payload.message || '', MAX_EXCERPT)
	} else if (eventName === 'permission_request') {
		if (payload.tool_use_id) line.tool_use_id = payload.tool_use_id
		line.tool_name = payload.tool_name || ''
		// Pass-through: emit tool_input verbatim as an object. Any
		// formatting (per-tool summary, truncation, JSON pretty-print)
		// is the consumer's job — keeps the hook stable when consumers
		// evolve or new tools need a custom render.
		if (payload.tool_input !== undefined) line.tool_input = payload.tool_input
	}
	// user_replied carries no extra fields — the event itself is the signal.

	return line
}

// Spawn a detached node child that tails the transcript looking for a
// "user clicked Cancel" pattern. The child runs ~60 s and exits silently
// if no cancel is detected. When detected, it appends a `tool_cancelled`
// event to the queue so the daemon can cancel the pending perm timer.
//
// Necessary because Claude Code has no "PermissionDenied" hook —
// PostToolUse doesn't fire on Cancel since the tool never runs.
function spawnCancelTailer(sid, transcriptPath) {
	if (!sid || !transcriptPath) return
	try {
		const { spawn } = require('child_process')
		const child = spawn(process.argv[0], [
			path.join(__dirname, 'tail-cancel.js'),
			sid,
			transcriptPath
		], { detached: true, stdio: 'ignore' })
		child.unref()
	} catch { /* swallow */ }
}

function main() {
	try {
		const arg = process.argv[2] || ''
		const eventName = ARG_TO_EVENT[arg]
		if (!eventName) return
		const payload = readStdin()
		if (!payload) return

		fs.mkdirSync(QUEUE_DIR, { recursive: true })

		const line = buildLine(eventName, payload)
		if (!line) return
		const file = path.join(QUEUE_DIR, `${line.sid}.jsonl`)
		fs.appendFileSync(file, JSON.stringify(line) + '\n')

		// Permission dialog opened → start tailing for a Cancel decision.
		if (eventName === 'permission_request') {
			spawnCancelTailer(line.sid, payload.transcript_path)
		}
	} catch {
		// swallow — emitter must never break Claude
	}
}

main()
