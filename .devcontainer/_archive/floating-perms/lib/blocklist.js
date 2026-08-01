// blocklist.js — patterns refused by /floating-perms batch regardless of
// what the user asks for. Small static list; if a pattern matches any
// entry, apply.js rejects it with a clear message.

const BLOCKED_BASH = new Set([
	'rm', 'rmdir',
	'sudo', 'su',
	'chmod', 'chown', 'chgrp',
	'mkfs', 'dd', 'mount', 'umount',
	'shutdown', 'reboot', 'halt', 'poweroff',
	'kill', 'killall', 'pkill',
	'iptables', 'nft', 'ufw',
	'eval', 'exec', 'source'
])

// Bash patterns that are technically allowed but only with arg restrictions.
// Right now: empty. Kept for future hardening where bare wildcards on
// some commands would need argv constraints (e.g. forcing https:// on curl).
const RESTRICTED_BASH = new Set([
])

const PATH_DENY_PREFIXES = [
	'/etc/**', '/boot/**', '/sys/**', '/proc/**',
	'/usr/sbin/**', '/usr/bin/**', '/sbin/**', '/bin/**',
	'/root/**',
	'/var/run/**', '/var/lib/docker/**'
]

function reasonForPattern(pattern) {
	if (typeof pattern !== 'string') return 'non-string pattern'
	const bash = pattern.match(/^Bash\(([^:]+):\*\)$/)
	if (bash) {
		const cmd = bash[1].trim()
		if (BLOCKED_BASH.has(cmd)) {
			return `command "${cmd}" is blocklisted (destructive or privilege-escalating)`
		}
		if (RESTRICTED_BASH.has(cmd)) {
			return `command "${cmd}" is restricted — specify args`
		}
		return null
	}
	const filey = pattern.match(/^(?:Edit|Write|Read|NotebookEdit)\((\/[^)]+)\)$/)
	if (filey) {
		const dir = filey[1]
		for (const deny of PATH_DENY_PREFIXES) {
			if (dir === deny || dir.startsWith(deny.replace(/\*\*$/, ''))) {
				return `path "${dir}" sits under a protected system root`
			}
		}
		return null
	}
	return 'unrecognized form (expected: Bash(cmd:*) or Tool(/path/**))'
}

function isBlocked(pattern) {
	return reasonForPattern(pattern) !== null
}

module.exports = { isBlocked, reasonForPattern }
