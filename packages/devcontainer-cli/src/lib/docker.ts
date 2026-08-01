// Docker orchestration: the base-image build and the rebuild-vs-reopen probe.

import { appendFileSync, mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { setEnvVar } from './env-file.js'
import type { Logger } from './logger.js'
import { relativeTo } from './paths.js'
import type { HostKind } from './platform.js'
import { toHostPath } from './platform.js'
import { runWithProgress } from './progress.js'
import { ancestry, hasCommand, PROXY_VARS, runCapture, tailFile } from './proc.js'

/**
 * Fallback Claude Code version when `.env` does not pin one.
 *
 * Lives here rather than in the command so session 5 (which owns the image and
 * the `<base-version>-cc<cc-version>` tag scheme) has one place to change.
 * Carried over from the dogfood script; the template copies still said 2.1.145,
 * which is the older of the two.
 */
export const DEFAULT_CLAUDE_CODE_VERSION = '2.1.220'

export function hasDocker(): boolean {
	return hasCommand('docker')
}

export function volumeCreate(name: string): void {
	// `|| true` in bash — an existing volume is the normal case, not an error.
	runCapture(['docker', 'volume', 'create', name])
}

export function imageExists(tag: string): boolean {
	return runCapture(['docker', 'image', 'inspect', tag]) !== null
}

export interface RebuildSignals {
	/** Rebuild the base image (cache allowed unless `noCache`). */
	requested: boolean
	/** Rebuild with `--no-cache`. */
	noCache: boolean
}

export interface DetectContext {
	hostKind: HostKind
	projectDir: string
	devcontainerDir: string
	/** `BUILD_BASE_NO_CACHE` as read from `.env` / the environment. */
	envNoCache: boolean
	logger: Logger
}

/**
 * Distinguish "Rebuild Container" from "Reopen in Container" from first-time.
 *
 * Ports `detect_no_cache_request` (initialize.sh:329-395) and the reasoning
 * recorded in its comments, which is worth restating because it is
 * counter-intuitive:
 *
 * VS Code passes **no** distinguishing flag. Rebuild and Reopen both invoke
 * `devContainersSpecCLI.js up` with identical arguments; the rebuild semantic
 * is that VS Code stops and removes the container *before* calling `up`. So the
 * container itself is the signal, not the command line.
 *
 * Two channels:
 *
 * - **Channel 1** — walk the process ancestry looking for an orchestrator
 *   carrying `--no-cache`. "Rebuild Without Cache" *does* propagate that flag.
 *   POSIX only, and already useless under WSL / Git Bash where the Windows-side
 *   VS Code process is unreachable — the bash version had the same limit and
 *   said so.
 * - **Channel 2** — probe for a container matching this workspace. `-a` is
 *   mandatory: Reopen stops the container before `initializeCommand` runs, so
 *   without it the probe misses an existing container and falsely rebuilds.
 */
export function detectRebuildSignals(context: DetectContext): RebuildSignals {
	const { logger } = context

	if (context.envNoCache) {
		logger.trace({ kind: 'decide', name: 'BUILD_BASE_NO_CACHE', value: '1', why: 'explicit env override' })
		return { requested: true, noCache: true }
	}

	// --- Channel 1 -----------------------------------------------------------
	const orchestrator = /(devcontainer|docker|compose|buildkit|code helper)/i
	const noCacheFlag = /(--build-no-cache|--no-cache)(\s|$)/
	const chain = ancestry(process.pid, 8)
	for (const [depth, info] of chain.entries()) {
		if (orchestrator.test(info.args) && noCacheFlag.test(info.args)) {
			logger.log(`  ↳ Detected --no-cache request (depth ${depth})`)
			logger.trace({ kind: 'decide', name: 'BUILD_BASE_NO_CACHE', value: '1', why: `ancestor depth ${depth}` })
			return { requested: true, noCache: true }
		}
	}

	// --- Channel 2 -----------------------------------------------------------
	if (!hasDocker()) return { requested: false, noCache: false }

	// VS Code writes these labels in host-native format (C:\… on Windows).
	// The POSIX form held here never matches on WSL / Git Bash, so translate.
	const localFolder = toHostPath(context.hostKind, context.projectDir)
	const configFile = toHostPath(context.hostKind, join(context.devcontainerDir, 'devcontainer.json'))
	// The compose-project filter excludes manually-run zombies: a bare
	// `docker run` carrying the devcontainer labels but no compose orchestration.
	const output = runCapture([
		'docker',
		'ps',
		'-a',
		'-q',
		'--filter',
		`label=devcontainer.local_folder=${localFolder}`,
		'--filter',
		`label=devcontainer.config_file=${configFile}`,
		'--filter',
		'label=com.docker.compose.project',
	])
	const containerId = (output ?? '').split('\n')[0]?.trim() ?? ''

	if (containerId.length === 0) {
		logger.log('  ↳ No matching devcontainer for this workspace — rebuild or first-time')
		logger.trace({ kind: 'decide', name: 'BUILD_BASE_REQUESTED', value: '1', why: 'no container matched labels' })
		return { requested: true, noCache: false }
	}
	logger.log(`  ↳ Devcontainer present (${containerId}, any state) — reopen, no base rebuild`)
	logger.trace({ kind: 'decide', name: 'BUILD_BASE_REQUESTED', value: '0', why: `container ${containerId} present` })
	return { requested: false, noCache: false }
}

export interface BuildBaseOptions {
	logger: Logger
	devcontainerDir: string
	envFile: string
	projectId: string
	version: string
	signals: RebuildSignals
	/** Whether `BUILD_BASE_NO_CACHE=1` came from `.env` and must be consumed. */
	envNoCacheFromFile: boolean
	timestamp: string
	dryRun: boolean
}

/** `claude-devcontainer-base:<cc-version>-<project-id>`. */
export function baseImageTag(version: string, projectId: string): string {
	return `claude-devcontainer-base:${version}-${projectId}`
}

/**
 * Build the base image when it is missing or when a rebuild was signalled.
 *
 * Ports `build_base_if_missing` (initialize.sh:412-494).
 */
export async function buildBaseIfMissing(options: BuildBaseOptions): Promise<void> {
	const { logger, devcontainerDir, signals } = options
	const tag = baseImageTag(options.version, options.projectId)

	// CLAUDE_CODE_VERSION is pinned by the caller, before the rebuild probe —
	// see the comment there for why the ordering is load-bearing.

	// Consume BUILD_BASE_NO_CACHE=1 when it came from .env, so the *next*
	// rebuild is a normal cached one. Without this the user is stuck in
	// permanent no-cache mode after toggling the flag once.
	if (signals.noCache && options.envNoCacheFromFile) {
		if (!options.dryRun) setEnvVar(options.envFile, 'BUILD_BASE_NO_CACHE', '0')
		logger.log('  ↳ Consumed BUILD_BASE_NO_CACHE=1 from .env (reset to 0 for next rebuild)')
	}

	// Bash reached this point with no docker and died inside `docker build`,
	// after having written CLAUDE_CODE_VERSION — so the .env pin above lands
	// either way. What it could not do is say why: the user got
	// "docker: command not found" buried in a build log. A host with no docker
	// cannot bring up a devcontainer at all, so this stays fatal, and says so.
	if (!hasDocker()) {
		if (options.dryRun) {
			logger.log('⚠ [dry-run] docker not found on PATH — the base image build would fail here')
			return
		}
		logger.styled('1;31', '✗ docker not found on PATH — cannot build the base image')
		logger.log('  Install Docker Desktop (macOS / Windows) or Docker Engine (Linux), then retry.')
		throw new Error('docker not found on PATH')
	}

	if (imageExists(tag)) {
		if (!signals.requested) {
			logger.log(`✓ Base image ${tag} present (no rebuild signal — skipping)`)
			return
		}
		logger.log(
			signals.noCache
				? `→ ${tag} exists but --no-cache requested — full rebuild`
				: `→ ${tag} exists but Rebuild Container detected — cached rebuild`,
		)
	}

	const logDir = join(devcontainerDir, 'logs')
	const logPath = join(logDir, `build-base-${options.version}-${options.timestamp}.log`)

	if (options.dryRun) {
		logger.log(`▸ [dry-run] would build ${tag} (log: ${relativeTo(devcontainerDir, logPath)})`)
		return
	}

	mkdirSync(logDir, { recursive: true })
	// Metadata header goes to the log only, keeping the banner clean.
	const proxyLines = PROXY_VARS.filter((key) => process.env[key] !== undefined)
		.map((key) => `    ${key}=${process.env[key] ?? ''}`)
		.join('\n')
	appendFileSync(
		logPath,
		[
			`=== build-base ${new Date().toString()} ===`,
			`version    : ${options.version}`,
			`tag        : ${tag}`,
			`dockerfile : ${join(devcontainerDir, 'Dockerfile.base')}`,
			`context    : ${devcontainerDir}`,
			`docker     : ${(runCapture(['docker', '--version']) ?? 'unknown').trim()}`,
			`host arch  : ${process.arch}`,
			'proxy env (will be stripped from docker build) :',
			proxyLines.length > 0 ? proxyLines : '    (none set)',
			'---',
			'',
		].join('\n'),
		'utf8',
	)

	const argv = ['docker', 'build', '--progress=plain']
	if (signals.noCache) {
		argv.push('--no-cache')
		logger.log('  (BUILD_BASE_NO_CACHE=1 → forcing full rebuild, no layer reuse)')
	}
	argv.push(
		'-f',
		join(devcontainerDir, 'Dockerfile.base'),
		'--build-arg',
		`CLAUDE_CODE_VERSION=${options.version}`,
		'-t',
		tag,
		devcontainerDir,
	)

	logger.trace({ kind: 'spawn', argv, cwd: devcontainerDir, unsetEnv: PROXY_VARS })
	const started = Date.now()
	const code = await runWithProgress({
		logger,
		argv,
		cwd: devcontainerDir,
		unsetEnv: PROXY_VARS,
		logPath,
		title: `Building Claude Devcontainer Base v${options.version}  (log: ${relativeTo(devcontainerDir, logPath)})`,
		size: 10,
	})
	const elapsed = Math.round((Date.now() - started) / 1000)
	logger.trace({ kind: 'exit', argv, code, ms: Date.now() - started })

	if (code === 0) {
		logger.styled('1;32', `✓ Built ${tag} in ${elapsed}s — log: ${logPath}`)
		return
	}
	logger.styled('1;31', `✗ Build failed (exit ${code}, ${elapsed}s elapsed). Full log: ${logPath}`)
	logger.log('  Tail of last 20 log lines :')
	for (const line of tailFile(logPath, 20)) logger.log(`    ${line}`)
	throw new Error(`base image build failed (exit ${code})`)
}
