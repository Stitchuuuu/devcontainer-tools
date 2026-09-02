#!/usr/bin/env node
// Drive a host Chromium over the DevTools Protocol — screenshot the running
// app, log in, and read real values back out of the page.
//
// Zero dependencies : Node 24 ships a global `WebSocket` and CDP is just JSON
// over one. That is not a stylistic choice — the devcontainer firewall blocks
// playwright / puppeteer on the registry and every Chromium CDN, and the repo
// bans per-platform native binaries (see #npm-optional-deps).
//
// The browser runs on the HOST (`wtf browser`, windowed), this script runs in
// the container and reaches it through the `host:9222` direct-TCP allowance.
//
// USAGE (from wtf) : `wtf shot 'https://symptems.localhost:55240/doctor/fr/' --out .tmp/shots/app.png --login`
// USAGE (direct)   : `node .devcontainer/claude/scripts/cdp.mjs shot <url> --out <path> [--login]`
//                    `node .devcontainer/claude/scripts/cdp.mjs login [--email …] [--password …]`
//                    `node .devcontainer/claude/scripts/cdp.mjs eval '<js>'`
//
// Two behaviours that look like bugs but are deliberate :
//   - the viewport override is left in place on exit, so a follow-up
//     `eval 'getComputedStyle(…)'` measures the same viewport that was just
//     captured. That is the whole point of the loop ;
//   - `login` always reloads the document. Setting the cookie is not enough —
//     see the block comment on doLogin().

import { lookup } from 'node:dns/promises'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { isIP } from 'node:net'
import { dirname, resolve } from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { BOLD, DIM, GREEN, RED, RESET, YELLOW } from '../../../scripts/lib/colors.mjs'

// ── constants ─────────────────────────────────────────────────────────────

const DEFAULT_HOST = 'host.docker.internal'
const DEFAULT_PORT = 9222
const DEFAULT_WIDTH = 1440
const DEFAULT_HEIGHT = 900

// Named viewports, because "does it still work on a laptop" is a question that
// gets asked on every UI change and nobody remembers the numbers. These are
// VIEWPORT sizes, not screen sizes : a maximised window on a 1080p display
// leaves ~950px once the browser chrome is subtracted, and a 13-14" laptop ~800.
// `macbook` is a recent MacBook (1512x982 screen) running the browser
// full-screen : 789px of viewport once the chrome is subtracted. It is the
// shortest of the three and therefore the one that decides whether a modal's
// max-height cap engages — capture it before declaring anything "fits".
const DEVICES = {
	desktop: { width: 1920, height: 950 },
	laptop: { width: 1440, height: 800 },
	macbook: { width: 1512, height: 789 },
}
const DEFAULT_EMAIL = 'admin@test.test'
const DEFAULT_PASSWORD = 'password'
const APP_HOST = 'symptems.localhost'
const APP_URL = `https://${APP_HOST}:55240/doctor/fr/`

const HTTP_TIMEOUT_MS = 5000 // /json/version probe — one LAN hop, fail fast
const CMD_TIMEOUT_MS = 15000 // any single CDP command
const LOAD_TIMEOUT_MS = 30000 // navigation + load event
const RENDER_TIMEOUT_MS = 10000 // the DOM predicate poll
const POLL_INTERVAL_MS = 100
const SETTLE_MS = 400 // Naive UI's drawer slide is ~300ms

const EXIT_OK = 0
const EXIT_FAIL = 1
const EXIT_USAGE = 2 // repo convention : 2 is CLI misuse
const EXIT_UNREACHABLE = 3
const EXIT_NAV = 4
const EXIT_TIMEOUT = 5
const EXIT_AUTH = 6
const EXIT_EVAL = 7
const EXIT_PROTOCOL = 8

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url))
// Two copies of the same file, and the difference between them is diagnostic.
// BAKED is what actually governs iptables : .devcontainer/Dockerfile COPYs the
// firewall tree into the image, so init-firewall.sh reads from /etc and never
// sees an edit made in the workspace. SOURCE is what a human just edited.
// BAKED ≠ SOURCE therefore means "edited but not rebuilt yet".
// ports.txt was called direct-tcp-allow.txt until 2026-08-10. Both names are
// read for one version — a project that has not renamed its file must still
// get the diagnostic below, not a silent "allowlist unreadable".
const portsFile = (dir) => {
	for (const name of ['ports.txt', 'direct-tcp-allow.txt']) {
		if (existsSync(resolve(dir, name))) return resolve(dir, name)
	}
	return resolve(dir, 'ports.txt')
}
const ALLOW_FILE_BAKED = portsFile('/etc/devcontainer-firewall')
const ALLOW_FILE_SOURCE = portsFile(resolve(SCRIPT_DIR, '../../firewall'))
// Written by .devcontainer/initialize.sh, the only code that runs on the host.
// From in here the kernel only tells us the hypervisor, not the host OS — see
// scripts/install-cross-arch-natives.mjs, which reads the same marker.
const HOST_OS_FILE = resolve(SCRIPT_DIR, '../../logs/host-os')

const USE_COLOR = process.stdout.isTTY && process.env.NO_COLOR !== '1'
const C = USE_COLOR
	? { bold: BOLD, dim: DIM, green: GREEN, red: RED, yellow: YELLOW, reset: RESET }
	: { bold: '', dim: '', green: '', red: '', yellow: '', reset: '' }

/**
 * The predicate polled by waitForRender(), evaluated in the page.
 *
 * `Page.loadEventFired` is structurally insufficient for this app :
 * `services/doctor/src/App.vue` wraps its whole template in
 * `<template v-if="auth.status === 'ready'">`, and `status` only leaves
 * `'idle'` once `onMounted(() => auth.init())` has completed an `/auth/me`
 * round-trip. So there is a real window — one HTTPS round-trip wide — where
 * the document is loaded, `#app` exists, and it is empty.
 *
 * Returns a *diagnosis*, never a bare boolean : on timeout the last `why` is
 * what says whether the app never mounted, never authenticated, or is still
 * loading fonts. `view` doubles as a machine-checkable read of the auth state
 * — `'layout'` proves the session cookie took, with no cookie inspection.
 */
const RENDER_PREDICATE = `(() => {
	const app = document.getElementById('app')
	if (!app) return { ok: false, why: 'no #app element — dev-server 503 page, or the wrong URL' }
	const root = app.querySelector('.login, .layout')
	if (!root) return { ok: false, why: 'app mounted but nothing rendered — auth.status is not "ready" yet' }
	if (root.getBoundingClientRect().height < 8) return { ok: false, why: 'root rendered with zero height' }
	if (document.fonts && document.fonts.status !== 'loaded') return { ok: false, why: 'webfonts still loading' }
	for (const img of document.images) if (!img.complete) return { ok: false, why: 'images still loading' }
	return { ok: true, view: root.classList.contains('layout') ? 'layout' : 'login' }
})()`

// ── errors ────────────────────────────────────────────────────────────────

/** An error that already carries the exit code the CLI should die with. */
class CdpError extends Error {
	/**
	 * @param {string} message
	 * @param {number} code - one of the EXIT_* constants.
	 */
	constructor(message, code) {
		super(message)
		this.name = 'CdpError'
		this.code = code
	}
}

// ── transport ─────────────────────────────────────────────────────────────

/**
 * Resolve the CDP endpoint to a **literal IP**.
 *
 * Chrome's anti-DNS-rebinding guard rejects any request whose `Host` header is
 * a DNS name — only `localhost` and IP literals pass, everything else gets a
 * 403 `Host header is specified and is not an IP address or localhost`.
 * `host.docker.internal` is a DNS name, so it is turned into an address here,
 * once, before anything touches the wire. family:4 on purpose — an AAAA answer
 * would produce `http://[::1]:9222` style URLs for nothing.
 * @returns {Promise<{ip: string, port: number, httpBase: string}>}
 */
async function resolveEndpoint() {
	const host = process.env.CDP_HOST || DEFAULT_HOST
	const port = Number(process.env.CDP_PORT || DEFAULT_PORT)
	if (isIP(host)) return { ip: host, port, httpBase: `http://${host}:${port}` }
	try {
		const { address } = await lookup(host, { family: 4 })
		return { ip: address, port, httpBase: `http://${address}:${port}` }
	} catch (err) {
		throw new CdpError(`cannot resolve ${host} : ${err.code ?? err.message}`, EXIT_UNREACHABLE)
	}
}

/**
 * Swap the authority of a CDP websocket URL for our resolved endpoint, keeping
 * the path (which carries the browser/target uuid) untouched.
 *
 * Chrome fills `webSocketDebuggerUrl` with whatever host it believes it serves
 * on — `localhost:9222` normally, `0.0.0.0:9222` with
 * `--remote-debugging-address=0.0.0.0`. Both are useless from inside the
 * container : `localhost` is the container itself, `0.0.0.0` is not an address
 * you connect to. **Only the path is trustworthy ; the authority is always
 * replaced.**
 * @param {string} wsUrl
 * @param {{ip: string, port: number}} ep
 * @returns {string}
 */
function rewriteCdpHost(wsUrl, ep) {
	const u = new URL(wsUrl)
	u.protocol = 'ws:'
	u.host = `${ep.ip}:${ep.port}`
	return u.href
}

/**
 * Does this allowlist file grant `host:<port>` ? `null` when the file cannot be
 * read at all, which is a different answer from "read it, port absent".
 * @param {string} file
 * @param {{ip: string, port: number}} ep
 * @returns {boolean | null}
 */
function allowsPort(file, ep) {
	try {
		const rules = readFileSync(file, 'utf8')
			.split('\n')
			.map(l => l.split('#')[0].replace(/\s/g, ''))
			.filter(Boolean)
		return rules.includes(`host:${ep.port}`) || rules.includes(`${ep.ip}:${ep.port}`)
	} catch {
		return null
	}
}

/**
 * The host OS, as recorded by initialize.sh — 'mac' | 'linux' | 'wsl' |
 * 'gitbash', or '' when the marker is missing (an older container that has not
 * been rebuilt since the marker was introduced).
 * @returns {string}
 */
function hostOs() {
	try {
		return readFileSync(HOST_OS_FILE, 'utf8').trim()
	} catch {
		return ''
	}
}

/**
 * ECONNREFUSED here is ambiguous, so explain both causes and rank them.
 *
 * The container's egress firewall REJECTs instantly when `host:9222` is absent
 * from ports.txt, and a Chromium that simply is not running looks
 * identical on the wire. The errno cannot discriminate — so read the allowlist,
 * which is the one side of the question observable from in here.
 * @param {{ip: string, port: number}} ep
 * @param {unknown} err
 * @returns {string}
 */
function unreachableHint(ep, err) {
	const code = err?.cause?.code ?? err?.code ?? err?.name ?? 'unknown'
	const baked = allowsPort(ALLOW_FILE_BAKED, ep)
	const source = allowsPort(ALLOW_FILE_SOURCE, ep)
	const head = `cannot reach the CDP endpoint at ${ep.ip}:${ep.port} (${code})`

	if (baked === false && source === true) {
		return `${head}
  → host:${ep.port} is allowed in .devcontainer/firewall/ports.txt but NOT in the
    image's baked copy (${ALLOW_FILE_BAKED}). The edit has not been applied yet.
  → Rebuild Container. Re-running init-firewall.sh is NOT enough — it recompiles from the
    baked copy and will not see your edit.`
	}
	if (baked === false) {
		return `${head}
  → the devcontainer firewall does not allow host:${ep.port}. Add it to
    .devcontainer/firewall/ports.txt, then Rebuild Container.`
	}
	if (code === 'TimeoutError' || code === 'ETIMEDOUT') {
		return `${head}
  → the connection hung rather than being refused. Check that Chromium was launched with
    --remote-debugging-address=0.0.0.0 and not just --remote-debugging-port.`
	}
	// On WSL2 the browser is a *Windows* process, so "not listening" has a second,
	// much more common cause than on mac : chrome.exe is running and visible, but
	// the Windows firewall is dropping 9222. Saying "run wtf browser" to someone
	// looking at an open browser window sends them the wrong way.
	if (hostOs() === 'wsl') {
		return `${head}
  → the firewall allows it, so nothing is answering on the Windows side.
  → if the Chromium window IS open, Windows Firewall is blocking chrome.exe on ${ep.port} —
    re-run \`wtf browser\` in the WSL2 shell and answer the prompt (private networks only).
  → otherwise run \`wtf browser\` on the HOST, in the WSL2 shell (it needs a window).`
	}
	return `${head}
  → the firewall allows it, so Chromium is not listening. Run \`wtf browser\` on the HOST
    (it needs a window ; it cannot run in this container).`
}

/**
 * Read /json/version and return the browser-level websocket URL, rewritten to
 * the literal-IP endpoint.
 * @param {{ip: string, port: number, httpBase: string}} ep
 * @returns {Promise<string>}
 */
async function fetchBrowserWsUrl(ep) {
	let res
	try {
		res = await fetch(`${ep.httpBase}/json/version`, { signal: AbortSignal.timeout(HTTP_TIMEOUT_MS) })
	} catch (err) {
		throw new CdpError(unreachableHint(ep, err), EXIT_UNREACHABLE)
	}
	if (!res.ok) {
		const body = (await res.text()).trim().slice(0, 200)
		// A 403 mentioning the Host header means the rewrite above was skipped
		// somewhere. Surface the body verbatim so it is recognised in seconds.
		throw new CdpError(`CDP /json/version → HTTP ${res.status} : ${body}`, EXIT_UNREACHABLE)
	}
	const info = await res.json()
	if (!info.webSocketDebuggerUrl) throw new CdpError('no webSocketDebuggerUrl in /json/version', EXIT_UNREACHABLE)
	return rewriteCdpHost(info.webSocketDebuggerUrl, ep)
}

/**
 * Open the websocket and return a minimal RPC surface. One socket for
 * everything : target routing rides on the flat `sessionId` field, so there is
 * never a second connection to manage.
 * @param {string} wsUrl
 * @returns {Promise<{send: Function, on: Function, waitFor: Function, close: Function}>}
 */
async function connect(wsUrl) {
	const ws = new WebSocket(wsUrl)
	const pending = new Map() // id → {method, resolve, reject, timer}
	const listeners = new Map() // method → Set<fn>
	let nextId = 0
	let dead = null

	await new Promise((res, rej) => {
		ws.addEventListener('open', res, { once: true })
		ws.addEventListener('error', () => rej(new CdpError(`websocket connect failed : ${wsUrl}`, EXIT_UNREACHABLE)), {
			once: true,
		})
	})

	ws.addEventListener('message', ev => {
		const msg = JSON.parse(ev.data)
		if (msg.id !== undefined) {
			const p = pending.get(msg.id)
			if (!p) return
			pending.delete(msg.id)
			clearTimeout(p.timer)
			if (msg.error) p.reject(new CdpError(`${p.method} : ${msg.error.message} (${msg.error.code})`, EXIT_PROTOCOL))
			else p.resolve(msg.result)
			return
		}
		// In flatten mode events carry `sessionId` at the top level too, so
		// handlers can ignore traffic coming from another target.
		for (const fn of listeners.get(msg.method) ?? []) fn(msg.params, msg.sessionId)
	})

	ws.addEventListener('error', () => {})
	ws.addEventListener('close', () => {
		dead = new CdpError('the browser closed the devtools connection', EXIT_UNREACHABLE)
		for (const p of pending.values()) {
			clearTimeout(p.timer)
			p.reject(dead)
		}
		pending.clear()
	})

	/**
	 * Send one CDP command. `sessionId` is omitted for browser-level domains
	 * (Target.*) and set for everything page-level.
	 * @param {string} method
	 * @param {object} [params]
	 * @param {string} [sessionId]
	 * @param {number} [timeoutMs]
	 * @returns {Promise<object>}
	 */
	const send = (method, params = {}, sessionId = undefined, timeoutMs = CMD_TIMEOUT_MS) =>
		new Promise((res, rej) => {
			if (dead) {
				rej(dead)
				return
			}
			const id = ++nextId
			const timer = setTimeout(() => {
				pending.delete(id)
				rej(new CdpError(`${method} timed out after ${timeoutMs}ms`, EXIT_TIMEOUT))
			}, timeoutMs)
			pending.set(id, { method, resolve: res, reject: rej, timer })
			ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }))
		})

	/**
	 * Register an event handler ; returns its own unsubscribe function.
	 * @param {string} method
	 * @param {(params: object, sessionId: string) => void} fn
	 * @returns {() => void}
	 */
	const on = (method, fn) => {
		if (!listeners.has(method)) listeners.set(method, new Set())
		listeners.get(method).add(fn)
		return () => listeners.get(method)?.delete(fn)
	}

	/**
	 * Promise for the next occurrence of an event.
	 *
	 * **Subscribe first, act second.** Create this promise BEFORE sending the
	 * command that triggers it : `Page.loadEventFired` regularly lands before
	 * `Page.navigate` resolves, and an await-then-subscribe ordering then hangs
	 * until the timeout for no reason at all.
	 * @param {string} method
	 * @param {string} [sessionId]
	 * @param {number} [timeoutMs]
	 * @returns {Promise<object>}
	 */
	const waitFor = (method, sessionId = undefined, timeoutMs = LOAD_TIMEOUT_MS) =>
		new Promise((res, rej) => {
			let timer
			const off = on(method, (params, sid) => {
				if (sessionId && sid !== sessionId) return
				clearTimeout(timer)
				off()
				res(params)
			})
			timer = setTimeout(() => {
				off()
				rej(new CdpError(`timed out after ${timeoutMs}ms waiting for ${method}`, EXIT_TIMEOUT))
			}, timeoutMs)
		})

	return { send, on, waitFor, close: () => ws.close() }
}

/**
 * Pick the tab to drive and attach a flat session to it.
 *
 * **Reuse over Target.createTarget**, deliberately : `eval` is useless on a
 * fresh tab (`window.seedCalendarDay` is installed on the app page), the
 * browser is windowed on purpose so the user watches this tab, and a new tab
 * per run piles up tabs while re-paying the whole boot cost every time.
 * @param {{send: Function}} conn
 * @returns {Promise<{targetId: string, sessionId: string, frameId: string}>}
 */
async function attachPage(conn) {
	const { targetInfos } = await conn.send('Target.getTargets')
	const pages = targetInfos.filter(t => t.type === 'page' && !t.url.startsWith('devtools://'))
	const preferred = pages.find(t => URL.parse(t.url)?.hostname.endsWith(APP_HOST))
	const targetId =
		(preferred ?? pages[0])?.targetId ?? (await conn.send('Target.createTarget', { url: 'about:blank' })).targetId

	// A headed Chromium does not paint background tabs : capturing an inactive
	// one returns a stale or empty frame. Activating is not cosmetic.
	await conn.send('Target.activateTarget', { targetId })

	// flatten:true → the session is addressed by a top-level `sessionId` on every
	// message, both directions, over this same socket. Without it each command
	// has to be tunnelled through Target.sendMessageToTarget and unwrapped out of
	// Target.receivedMessageFromTarget — same result, twice the JSON, deprecated.
	const { sessionId } = await conn.send('Target.attachToTarget', { targetId, flatten: true })

	await conn.send('Page.enable', {}, sessionId)
	await conn.send('Network.enable', {}, sessionId)
	// Independent of the launch flag : docker/proxy/dev.crt is a mkcert cert
	// whose root CA belongs to another machine, so without this every capture is
	// a Chrome interstitial instead of the app.
	await conn.send('Security.enable', {}, sessionId)
	await conn.send('Security.setIgnoreCertificateErrors', { ignore: true }, sessionId)

	const { frameTree } = await conn.send('Page.getFrameTree', {}, sessionId)
	return { targetId, sessionId, frameId: frameTree.frame.id }
}

// ── page helpers ──────────────────────────────────────────────────────────

/**
 * Turn a Runtime.exceptionDetails into something readable. A non-Error
 * rejection stringifies to `undefined` through the usual paths, which is
 * exactly the unreadable outcome to avoid.
 * @param {object} details
 * @returns {string}
 */
function describeException(details) {
	const ex = details?.exception
	return ex?.description ?? ex?.value ?? details?.text ?? 'unknown in-page exception'
}

/**
 * Evaluate an expression in the page and return its value.
 * @param {{send: Function}} conn
 * @param {string} sid
 * @param {string} expression
 * @returns {Promise<unknown>}
 */
async function evaluate(conn, sid, expression) {
	const r = await conn.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, sid)
	if (r.exceptionDetails) throw new CdpError(describeException(r.exceptionDetails), EXIT_EVAL)
	return r.result?.value
}

/**
 * Record the HTTP status of the **main document** response.
 *
 * A dev-server that is down is otherwise invisible : HAProxy answers 503 with a
 * well-formed HTML page (docker/proxy/error503.http), the load event fires on
 * schedule, and the capture is a pixel-perfect error page. Filtering on the
 * main frame id keeps subframes and XHRs out of it.
 * @param {{on: Function}} conn
 * @param {string} sid
 * @param {string} frameId
 * @returns {{status: number, url: string}}
 */
function trackDocumentStatus(conn, sid, frameId) {
	const seen = { status: 0, url: '' }
	conn.on('Network.responseReceived', (p, s) => {
		if (s !== sid || p.type !== 'Document' || p.frameId !== frameId) return
		seen.status = p.response.status // last one wins : redirect chains
		seen.url = p.response.url
	})
	return seen
}

/**
 * Turn a main-document HTTP status into something actionable.
 * @param {number} status
 * @param {string} url
 * @returns {string}
 */
function explainHttp(status, url) {
	if (status === 503) {
		return `${url} → 503. HAProxy is up but the rspack dev-server behind it is not.
  → run \`wtf dev\` on the HOST (it needs docker + tmux, neither exists in this container).`
	}
	if (status === 404) {
		return `${url} → 404. historyApiFallback did not match — check the /doctor/<locale>/ path shape.`
	}
	return `${url} → HTTP ${status}.`
}

/**
 * Poll the render predicate until it reports ok, then let the page settle.
 * @param {{send: Function}} conn
 * @param {string} sid
 * @param {{timeoutMs: number, settleMs: number}} o
 * @returns {Promise<{ok: true, view: string}>}
 */
async function waitForRender(conn, sid, o) {
	const deadline = Date.now() + o.timeoutMs
	let why = 'never evaluated'
	while (Date.now() < deadline) {
		const r = await evaluate(conn, sid, RENDER_PREDICATE)
		if (r?.ok) {
			// Naive UI animates : the drawer slides in over ~300ms and n-modal fades.
			// A settle beat is cheaper and far more robust than trying to detect the
			// end of a CSS transition.
			await sleep(o.settleMs)
			return r
		}
		why = r?.why ?? 'predicate returned nothing'
		await sleep(POLL_INTERVAL_MS)
	}
	throw new CdpError(`the page never rendered within ${o.timeoutMs}ms — last check : ${why}`, EXIT_TIMEOUT)
}

/**
 * Navigate and wait for the load event, failing loudly on net-stack errors and
 * on any main-document status >= 400.
 * @param {{send: Function, waitFor: Function, on: Function}} conn
 * @param {string} sid
 * @param {string} frameId
 * @param {string} url
 * @returns {Promise<void>}
 */
async function navigate(conn, sid, frameId, url) {
	const doc = trackDocumentStatus(conn, sid, frameId)
	const loaded = conn.waitFor('Page.loadEventFired', sid) // subscribe first
	const r = await conn.send('Page.navigate', { url }, sid)
	// Checked before awaiting the load event : on a net-stack failure that event
	// never fires, and waiting 30s for it hides the real cause.
	if (r.errorText) throw new CdpError(`navigation failed : ${r.errorText} (${url})`, EXIT_NAV)
	await loaded
	if (doc.status >= 400) throw new CdpError(explainHttp(doc.status, doc.url || url), EXIT_NAV)
}

/**
 * Reload the current document and wait for the load event.
 * @param {{send: Function, waitFor: Function}} conn
 * @param {string} sid
 * @returns {Promise<void>}
 */
async function reload(conn, sid) {
	const loaded = conn.waitFor('Page.loadEventFired', sid)
	await conn.send('Page.reload', {}, sid)
	await loaded
}

// ── login ─────────────────────────────────────────────────────────────────

/**
 * Build the in-page login expression.
 *
 * It MUST run in a page already on the app origin : the `sess` cookie is
 * `Domain=symptems.localhost; SameSite=lax; httpOnly`, so from about:blank the
 * request is an opaque cross-site fetch, the Set-Cookie is dropped, and you get
 * a cheerful 200 with no session.
 *
 * The body is written so it **can never reject** — every failure mode (network
 * error, TLS refusal, non-JSON body, 401, 422) comes back as a structured
 * value. Credentials are interpolated through JSON.stringify, never
 * concatenation, so a password containing a quote cannot break the parse.
 * @param {string} email
 * @param {string} password
 * @returns {string}
 */
function loginExpression(email, password) {
	return `(async () => {
	// https://symptems.localhost:55240 → https://api.symptems.localhost:55240,
	// the same derivation the app's API_URL constant hard-codes at build time.
	const api = location.origin.replace('://', '://api.')
	try {
		const res = await fetch(api + '/auth/login', {
			method: 'POST',
			credentials: 'include',
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
			body: JSON.stringify(${JSON.stringify({ email, password })}),
		})
		const text = await res.text()
		let body = null
		try {
			body = text ? JSON.parse(text) : null
		} catch (parseErr) {
			body = { error: 'non-JSON response', raw: text.slice(0, 300) }
		}
		return { ok: res.ok, status: res.status, body: body, api: api }
	} catch (e) {
		return { ok: false, status: 0, body: { error: String((e && e.message) || e) }, api: api }
	}
})()`
}

/**
 * Log in, four acts, none of them droppable :
 *   1. be on the app origin — navigate there if the tab is elsewhere ;
 *   2. POST from inside the page, so the BROWSER stores the httpOnly cookie
 *      (we never see it and never need to) ;
 *   3. **full document reload** — services/doctor/src/App.vue calls
 *      `auth.init()` once, from onMounted(), and init() early-returns unless
 *      `status === 'idle'` (services/shared/stores/auth.ts). Setting a cookie
 *      on a live page therefore changes exactly nothing on screen ; only a new
 *      document re-runs the /auth/me probe that flips isAuthenticated ;
 *   4. wait for render and read back which view came up.
 *
 * Idempotent : already authenticated (view === 'layout') → the POST is skipped.
 * @param {{send: Function, waitFor: Function, on: Function}} conn
 * @param {string} sid
 * @param {string} frameId
 * @param {{email: string, password: string, wait: number, settle: number}} o
 * @returns {Promise<{view: string, skipped: boolean, api?: string}>}
 */
async function doLogin(conn, sid, frameId, o) {
	const here = await evaluate(conn, sid, 'location.href')
	if (!URL.parse(String(here ?? ''))?.hostname.endsWith(APP_HOST)) {
		await navigate(conn, sid, frameId, APP_URL)
	}
	let rendered = await waitForRender(conn, sid, { timeoutMs: o.wait, settleMs: 0 })
	if (rendered.view === 'layout') return { view: 'layout', skipped: true }

	const r = await evaluate(conn, sid, loginExpression(o.email, o.password))
	if (!r?.ok) {
		const msg = r?.body?.error ?? 'no response'
		if (r?.status === 0) {
			throw new CdpError(
				`the page could not reach the API at ${r?.api} : ${msg}
  → is \`wtf dev\` running on the host ?`,
				EXIT_NAV,
			)
		}
		if (r?.status === 401) {
			throw new CdpError(
				`login refused (401) : ${msg}
  → the seeded credentials are admin@test.test / password (0000_create_users.sql).`,
				EXIT_AUTH,
			)
		}
		if (r?.status === 422) {
			const issues = (r?.body?.issues ?? []).map(i => `${i.path?.[0]?.key ?? '?'}: ${i.message}`).join(', ')
			throw new CdpError(`invalid login payload (422) : ${msg}${issues ? ` — ${issues}` : ''}`, EXIT_AUTH)
		}
		throw new CdpError(`the API answered ${r?.status} : ${msg}`, EXIT_NAV)
	}

	await reload(conn, sid)
	rendered = await waitForRender(conn, sid, { timeoutMs: o.wait, settleMs: o.settle })
	if (rendered.view !== 'layout') {
		throw new CdpError(
			`login returned 200 but the app still shows the ${rendered.view} view — the session cookie did not stick`,
			EXIT_AUTH,
		)
	}
	return { view: 'layout', skipped: false, api: r.api }
}

// ── commands ──────────────────────────────────────────────────────────────

/**
 * Navigate, optionally log in, then write a PNG.
 *
 * Order matters : metrics are overridden BEFORE navigating because Layout.vue
 * has a viewport-width `isMobile` branch, so overriding after load risks
 * capturing the mobile layout or a mid-recompute frame.
 * @param {{send: Function, waitFor: Function, on: Function}} conn
 * @param {string} sid
 * @param {string} frameId
 * @param {object} o
 * @returns {Promise<object>}
 */
async function cmdShot(conn, sid, frameId, o) {
	await conn.send(
		'Emulation.setDeviceMetricsOverride',
		{ width: o.width, height: o.height, deviceScaleFactor: o.scale, mobile: false },
		sid,
	)

	// Login FIRST, then navigate once. doLogin is self-sufficient — it only moves
	// the tab when it is off-origin, and skips the POST when the layout is already
	// up — so the target URL is loaded exactly once. Doing it the other way round
	// loaded the deep link, threw it away on the login reload, and loaded it
	// again : three renders and a visible double flash for the human watching the
	// headed browser.
	const login = o.login ? await doLogin(conn, sid, frameId, o) : null
	await navigate(conn, sid, frameId, o.url)
	const rendered = await waitForRender(conn, sid, { timeoutMs: o.wait, settleMs: o.settle })

	// --before : put the page into a state no url can express — a drawer opened
	// by a click, a hover, a style override for an A/B. It runs HERE, after the
	// render predicate and before the capture, so that navigation and mutation
	// stay in ONE invocation : a mutation issued as a separate `eval` call would
	// be applied to whatever page the tab happens to be on by then, which is the
	// staleness trap this tool exists to close. Settle again afterwards, since
	// what it triggers is usually an animation (a drawer slide is ~300ms).
	if (o.before) {
		await evaluate(conn, sid, o.before)
		await sleep(o.settle)
	}

	// A headed Chromium does not paint background tabs — re-activate right before
	// capturing in case the user clicked another tab while we were waiting.
	await conn.send('Target.activateTarget', { targetId: o.targetId })
	// If a --full capture ever comes back clipped, the fix is not a CDP mystery :
	// Page.getLayoutMetrics → cssContentSize.height → re-issue the metrics
	// override at that height → capture → restore.
	const shot = await conn.send(
		'Page.captureScreenshot',
		{ format: 'png', captureBeyondViewport: Boolean(o.full) },
		sid,
		LOAD_TIMEOUT_MS,
	)
	const buf = Buffer.from(shot.data, 'base64')
	mkdirSync(dirname(o.out), { recursive: true })
	writeFileSync(o.out, buf)
	return { out: o.out, bytes: buf.length, view: rendered.view, width: o.width, height: o.height, full: !!o.full, login }
}

/**
 * Evaluate arbitrary JS in the page and return its value.
 * @param {{send: Function}} conn
 * @param {string} sid
 * @param {object} o
 * @returns {Promise<object>}
 */
async function cmdEval(conn, sid, o) {
	return { value: await evaluate(conn, sid, o.expression) }
}

// ── cli ───────────────────────────────────────────────────────────────────

const USAGE = `cdp.mjs — drive a host Chromium over the DevTools Protocol.

  node .devcontainer/claude/scripts/cdp.mjs shot <url> --out <path> [options]
  node .devcontainer/claude/scripts/cdp.mjs login [--email <e>] [--password <p>]
  node .devcontainer/claude/scripts/cdp.mjs eval <js>
  node .devcontainer/claude/scripts/cdp.mjs reset

Options
  --out <path>       PNG destination (required by shot ; parent dirs created)
  --device <name>    viewport preset — ${Object.entries(DEVICES)
		.map(([k, v]) => `${k} (${v.width}x${v.height})`)
		.join(', ')}.
                     --width / --height still win when given explicitly.
  --width <n>        viewport width, default ${DEFAULT_WIDTH}
  --height <n>       viewport height, default ${DEFAULT_HEIGHT}
  --full             capture the whole page, not just the viewport
  --scale <n>        device pixel ratio, default 1. Use 2 to compare against a
                     Figma export : those render at 2x, and a 1.2px stroke has
                     no fully-covered pixel at 1x, so stroke weight and colour
                     read lighter than they are.
  --login            log in first, then capture (idempotent)
  --email <e>        default ${DEFAULT_EMAIL}
  --password <p>     default ${DEFAULT_PASSWORD}
  --wait <ms>        render-predicate timeout, default ${RENDER_TIMEOUT_MS}
  --settle <ms>      pause after render, default ${SETTLE_MS} (drawer/modal animation)
  --before <js>      JS to run after render and before capture, for a state no url
                     can express — open a drawer with a click, inject a style
                     override for an A/B. Same invocation as the navigation on
                     purpose : a separate \`eval\` would hit whatever page the tab
                     has drifted to. Settles again afterwards.
  --json             one JSON blob on stdout, no human output
  --                 stop flag parsing (use for \`eval -- '-1'\`)

Environment
  CDP_HOST           default ${DEFAULT_HOST}
  CDP_PORT           default ${DEFAULT_PORT}

\`reset\` clears the viewport emulation that \`shot\` deliberately leaves behind —
run it when you are done measuring, otherwise the window keeps rendering at the
captured size and anything taller than the real window looks cropped.

Requires \`wtf dev\` AND \`wtf browser\` running on the HOST.
A deep link into an authenticated-only view renders nothing while logged out —
Layout.vue only mounts once authenticated, and the query string is kept but
acted on by nobody — so always pass --login with one.
\`eval 'window.seedCalendarDay()'\` likewise needs a logged-in page : the sync
store is only initialised from Layout.vue.
`

/**
 * Hand-rolled argv parsing — the repo does not use node:util parseArgs.
 * @param {string[]} argv
 * @returns {object}
 */
function parseArgs(argv) {
	const o = {
		cmd: '',
		url: '',
		out: '',
		expression: '',
		before: '',
		width: DEFAULT_WIDTH,
		height: DEFAULT_HEIGHT,
		full: false,
		login: false,
		email: DEFAULT_EMAIL,
		password: DEFAULT_PASSWORD,
		wait: RENDER_TIMEOUT_MS,
		settle: SETTLE_MS,
		scale: 1,
		device: '',
		json: false,
	}
	const positionals = []
	const seen = new Set()
	const values = {
		'--out': 'out',
		'--email': 'email',
		'--password': 'password',
		'--device': 'device',
		'--before': 'before',
	}
	const numbers = {
		'--width': 'width',
		'--height': 'height',
		'--wait': 'wait',
		'--settle': 'settle',
		'--scale': 'scale',
	}
	let rest = false

	for (let i = 2; i < argv.length; i++) {
		const a = argv[i]
		if (!rest) seen.add(a)
		if (rest) {
			positionals.push(a)
		} else if (a === '--') {
			rest = true
		} else if (values[a]) {
			o[values[a]] = argv[++i]
		} else if (numbers[a]) {
			o[numbers[a]] = Number(argv[++i])
		} else if (a === '--full') {
			o.full = true
		} else if (a === '--login') {
			o.login = true
		} else if (a === '--json') {
			o.json = true
		} else if (a === '--help' || a === '-h') {
			process.stdout.write(USAGE)
			process.exit(EXIT_OK)
		} else if (a.startsWith('-')) {
			process.stderr.write(`unknown arg: ${a}\n`)
			process.exit(EXIT_USAGE)
		} else {
			positionals.push(a)
		}
	}

	// A preset only fills in what was not asked for explicitly, so
	// `--device laptop --width 1280` narrows a laptop rather than being ignored.
	if (o.device) {
		const d = DEVICES[o.device]
		if (d) {
			if (!seen.has('--width')) o.width = d.width
			if (!seen.has('--height')) o.height = d.height
		}
	}
	o.cmd = positionals[0] ?? ''
	if (o.cmd === 'shot') o.url = positionals[1] ?? ''
	if (o.cmd === 'eval') o.expression = positionals.slice(1).join(' ')
	return o
}

/**
 * Validate the parsed options, exiting 2 on misuse.
 * @param {object} o
 * @returns {void}
 */
function validate(o) {
	const bad = m => {
		process.stderr.write(`${m}\n\n${USAGE}`)
		process.exit(EXIT_USAGE)
	}
	if (!o.cmd) bad('missing subcommand')
	if (!['shot', 'login', 'eval', 'reset'].includes(o.cmd)) bad(`unknown subcommand: ${o.cmd}`)
	if (o.cmd === 'shot' && !o.url) bad('shot needs a <url>')
	if (o.cmd === 'shot' && !o.out) bad('shot needs --out <path>')
	if (o.cmd === 'eval' && !o.expression) bad('eval needs a <js> expression')
	if (o.device && !DEVICES[o.device]) bad(`unknown --device: ${o.device} (known: ${Object.keys(DEVICES).join(', ')})`)
	for (const k of ['width', 'height', 'wait', 'settle']) {
		if (!Number.isFinite(o[k])) bad(`--${k} must be a number`)
	}
}

/**
 * Human-readable rendering of a command result.
 * @param {string} cmd
 * @param {object} out
 * @returns {void}
 */
function report(cmd, out) {
	if (cmd === 'shot') {
		const kb = Math.round(out.bytes / 1024)
		const auth = out.login ? (out.login.skipped ? ' (already logged in)' : ' (logged in)') : ''
		process.stdout.write(
			`${C.green}✓${C.reset} ${out.out} ${C.dim}(${kb} KB, ${out.width}×${out.height}${out.full ? ', full page' : ''}, view: ${out.view}${auth})${C.reset}\n`,
		)
		return
	}
	if (cmd === 'login') {
		const how = out.skipped ? 'already authenticated' : 'authenticated'
		process.stdout.write(`${C.green}✓${C.reset} ${how} ${C.dim}(view: ${out.view})${C.reset}\n`)
		return
	}
	if (cmd === 'reset') {
		process.stdout.write(
			`${C.green}✓${C.reset} viewport emulation cleared ${C.dim}(the window now renders at its own ${out.inner.w}×${out.inner.h})${C.reset}\n`,
		)
		return
	}
	process.stdout.write(`${JSON.stringify(out.value, null, 2)}\n`)
}

/**
 * Drop the viewport emulation and hand the tab back to the human.
 *
 * `shot` deliberately leaves `Emulation.setDeviceMetricsOverride` in place so a
 * follow-up `eval 'getComputedStyle(…)'` measures the very viewport that was
 * captured. The cost is that the window then renders at the emulated size rather
 * than its own : a 900px-tall override inside an 857px-tall window hides 43px
 * under the edge, and everything looks cropped and off-centre while the page is
 * in fact fine. This is the undo.
 * @param {{send: Function}} conn
 * @param {string} sid
 * @returns {Promise<{cleared: true, inner: object}>}
 */
async function cmdReset(conn, sid) {
	// Take ownership before releasing. An override survives the session that set
	// it — `shot` closes its websocket and the emulation stays on the target — and
	// `clearDeviceMetricsOverride` from a fresh session is then a no-op, because
	// this session holds no override to clear. Setting one first makes it ours,
	// and clearing ours drops the target back to the real window. Verified : a
	// bare clear left innerHeight at the captured 600, this restores it.
	await conn.send(
		'Emulation.setDeviceMetricsOverride',
		{ width: 0, height: 0, deviceScaleFactor: 0, mobile: false },
		sid,
	)
	await conn.send('Emulation.clearDeviceMetricsOverride', {}, sid)
	const inner = await evaluate(conn, sid, 'JSON.stringify({w: innerWidth, h: innerHeight})')
	return { cleared: true, inner: JSON.parse(String(inner ?? '{}')) }
}

async function main() {
	const o = parseArgs(process.argv)
	validate(o)

	const ep = await resolveEndpoint()
	const conn = await connect(await fetchBrowserWsUrl(ep))
	try {
		const { targetId, sessionId, frameId } = await attachPage(conn)
		o.targetId = targetId
		let out
		switch (o.cmd) {
			case 'shot': {
				out = await cmdShot(conn, sessionId, frameId, o)
				break
			}
			case 'login': {
				out = await doLogin(conn, sessionId, frameId, o)
				break
			}
			case 'reset': {
				out = await cmdReset(conn, sessionId)
				break
			}
			default: {
				out = await cmdEval(conn, sessionId, o)
				break
			}
		}
		if (o.json) process.stdout.write(`${JSON.stringify(out, null, 2)}\n`)
		else report(o.cmd, out)
	} finally {
		conn.close()
	}
}

main()
	.then(() => process.exit(EXIT_OK))
	.catch(err => {
		// Errors go to stderr even under --json, so the single stdout blob is never
		// half a JSON document.
		process.stderr.write(`${C.red}cdp:${C.reset} ${err?.message ?? err}\n`)
		process.exit(err instanceof CdpError ? err.code : EXIT_FAIL)
	})
