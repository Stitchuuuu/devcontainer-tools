// cleanup.js — revoker for SessionEnd and TTL expiry.
//
// revokeBy(predicate, reason) atomically:
//   1. removes matching grants from state.grants
//   2. removes their patterns from settings.local.json `permissions.allow`
//      (only if no other still-active grant uses the same pattern)
//   3. appends one audit line listing what was revoked.

const { withState, readAllow, writeAllow, audit } = require('./lib/state')

function patternsStillReferenced(remainingGrants, pattern) {
	return remainingGrants.some(g => g.pattern === pattern)
}

function revokeBy(predicate, reason) {
	return withState((state) => {
		const matched = state.grants.filter(predicate)
		if (matched.length === 0) return undefined

		const remaining = state.grants.filter(g => !predicate(g))
		state.grants = remaining

		const { settings, allow } = readAllow()
		const toDrop = new Set()
		for (const g of matched) {
			if (!patternsStillReferenced(remaining, g.pattern)) toDrop.add(g.pattern)
		}
		const newAllow = allow.filter(p => !toDrop.has(p))
		if (newAllow.length !== allow.length) {
			writeAllow(settings, newAllow)
		}

		audit('revoke', {
			reason,
			count: matched.length,
			patterns: matched.map(g => ({
				pattern: g.pattern,
				sid: g.sid,
				granted_at: g.granted_at,
				expires_at: g.expires_at
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
