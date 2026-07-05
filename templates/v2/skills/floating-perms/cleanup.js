// cleanup.js — revoker for SessionEnd and TTL expiry.
//
// revokeBy(predicate, reason) atomically:
//   1. removes matching grants from state.grants
//   2. removes their patterns from settings.local.json `permissions.allow`
//      (only if no other still-active grant uses the same pattern)
//   3. removes their `additional_dir` entry from settings.local.json
//      `permissions.additionalDirectories` under the same "no still-active
//      grant uses it" rule
//   4. appends one audit line listing what was revoked.

const { withState, readAllow, writeAllow, audit } = require('./lib/state')

function patternsStillReferenced(remainingGrants, pattern) {
	return remainingGrants.some(g => g.pattern === pattern)
}

function dirStillReferenced(remainingGrants, dir) {
	return remainingGrants.some(g => g.additional_dir === dir)
}

function revokeBy(predicate, reason) {
	return withState((state) => {
		const matched = state.grants.filter(predicate)
		if (matched.length === 0) return undefined

		const remaining = state.grants.filter(g => !predicate(g))
		state.grants = remaining

		const { settings, allow, additionalDirectories } = readAllow()
		const toDrop = new Set()
		for (const g of matched) {
			if (!patternsStillReferenced(remaining, g.pattern)) toDrop.add(g.pattern)
		}
		const dirsToDrop = new Set()
		for (const g of matched) {
			if (g.additional_dir && !dirStillReferenced(remaining, g.additional_dir)) {
				dirsToDrop.add(g.additional_dir)
			}
		}
		const newAllow = allow.filter(p => !toDrop.has(p))
		const newAdditionalDirs = additionalDirectories.filter(d => !dirsToDrop.has(d))
		const remainingFloating = remaining.map(g => g.pattern)
		const allowChanged = newAllow.length !== allow.length || remainingFloating.length === 0
		const dirsChanged = newAdditionalDirs.length !== additionalDirectories.length
		if (allowChanged || dirsChanged) {
			// Pass the dirs param only if we touched it, so we don't
			// churn additionalDirectories when nothing floating-side
			// changed there.
			writeAllow(settings, newAllow, remainingFloating,
				dirsChanged ? newAdditionalDirs : undefined)
		}

		audit('revoke', {
			reason,
			count: matched.length,
			patterns: matched.map(g => ({
				pattern: g.pattern,
				sid: g.sid,
				granted_at: g.granted_at,
				expires_at: g.expires_at,
				additional_dir: g.additional_dir || null
			}))
		})

		return { state, result: matched }
	})
}

function revokeForSession(sid) {
	if (!sid) return []
	return revokeBy(g => g.sid === sid, 'session_end') || []
}

function revokeExpired(now) {
	const t = now || Date.now()
	return revokeBy(
		g => typeof g.expires_at === 'number' && g.expires_at < t,
		'ttl_expired'
	) || []
}

function revokeManual(pattern) {
	if (!pattern) return []
	return revokeBy(g => g.pattern === pattern, 'manual_revoke') || []
}

function revokeOrphans(currentSid) {
	if (!currentSid) return []
	return revokeBy(
		g => g.sid !== currentSid,
		'gc_orphans'
	) || []
}

module.exports = { revokeForSession, revokeExpired, revokeManual, revokeOrphans }
