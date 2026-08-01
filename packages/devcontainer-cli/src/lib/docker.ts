// Docker orchestration: volume/image helpers and the rebuild-vs-reopen probe.
// The base image is not built here — compose pulls the published
// ghcr.io/stitchuuuu/devcontainer-base tag, and bumping that tag is what an
// upgrade means.

import { join } from 'node:path'
import type { Logger } from './logger.js'
import type { HostKind } from './platform.js'
import { toHostPath } from './platform.js'
import { hasCommand, runCapture } from './proc.js'

/**
 * Fallback Claude Code version when `.env` does not pin one.
 *
 * Lives here rather than in the command so there is one place to change when
 * the pin moves. The published matrix is owned by the devcontainer-base repo
 * (`cc-versions.json`, tag scheme `<base-version>-cc<cc-version>`); this
 * fallback must name a version that repo publishes.
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
	/** True when no container matches this workspace — rebuild or first-time. */
	requested: boolean
}

export interface DetectContext {
	hostKind: HostKind
	projectDir: string
	devcontainerDir: string
	logger: Logger
}

/**
 * Distinguish "Rebuild Container" from "Reopen in Container" from first-time.
 *
 * Ports the container-presence half of `detect_no_cache_request`
 * (initialize.sh:329-395), whose reasoning is worth restating because it is
 * counter-intuitive:
 *
 * VS Code passes **no** distinguishing flag. Rebuild and Reopen both invoke
 * `devContainersSpecCLI.js up` with identical arguments; the rebuild semantic
 * is that VS Code stops and removes the container *before* calling `up`. So the
 * container itself is the signal, not the command line. `-a` is mandatory:
 * Reopen stops the container before `initializeCommand` runs, so without it the
 * probe misses an existing container and falsely reports a rebuild.
 *
 * The probe's outcome is informational now that no local build hangs off it —
 * it feeds the log line that tells a human (and rebuild-debug traces) which of
 * the three states VS Code is in.
 */
export function detectRebuildSignals(context: DetectContext): RebuildSignals {
	const { logger } = context

	if (!hasDocker()) return { requested: false }

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
		return { requested: true }
	}
	logger.log(`  ↳ Devcontainer present (${containerId}, any state) — reopen, no base rebuild`)
	logger.trace({ kind: 'decide', name: 'BUILD_BASE_REQUESTED', value: '0', why: `container ${containerId} present` })
	return { requested: false }
}

/**
 * `claude-devcontainer-base:<cc-version>-<project-id>` — the legacy local tag.
 *
 * Still traced so a host that has not switched onto the published image can be
 * diagnosed; the dogfood's escape hatch (`BASE_IMAGE` in `.env`) names it.
 */
export function baseImageTag(version: string, projectId: string): string {
	return `claude-devcontainer-base:${version}-${projectId}`
}
