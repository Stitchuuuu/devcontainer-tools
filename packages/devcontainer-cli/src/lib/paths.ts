// Locating the .devcontainer directory.
//
// initialize.sh could take a shortcut here: `DEVCONTAINER_DIR="$(cd "$(dirname
// "$0")" && pwd)"`. The script lived inside the directory it operated on, so
// its own location was the answer.
//
// That shortcut dies with the port. Under `npx @stitchu/devcontainer-cli`, the
// entry point sits in an npm cache directory that has nothing to do with the
// project, so location has to be resolved from the working directory instead —
// or stated outright, which is also what makes the differential test possible.

import { existsSync, statSync } from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'

export interface ProjectPaths {
	/** Absolute path of `.devcontainer/`. */
	devcontainerDir: string
	/** Absolute path of its parent, the workspace root. */
	projectDir: string
	/** `<devcontainerDir>/.env`. */
	envFile: string
}

export class PathResolutionError extends Error {}

/**
 * Resolve the pair of directories every command works against.
 *
 * Accepts, in order of precedence:
 * 1. An explicit `--devcontainer-dir` (used by tests and by anyone driving the
 *    CLI from outside the project).
 * 2. `cwd` when it is itself named `.devcontainer`.
 * 3. `cwd/.devcontainer`.
 *
 * Walking up the tree looking for a `.devcontainer` ancestor is deliberately
 * *not* implemented: `devc initialize` mutates `.env`, seeds firewall files and
 * kicks off an image build, and picking the wrong project silently would be
 * worse than an error message.
 */
export function resolveProjectPaths(cwd: string, explicitDir?: string): ProjectPaths {
	const devcontainerDir = resolve(explicitDir ?? implicitDevcontainerDir(cwd))
	if (!existsSync(devcontainerDir) || !statSync(devcontainerDir).isDirectory()) {
		throw new PathResolutionError(
			`No .devcontainer directory at ${devcontainerDir}\n` +
				`  Run devc from a project root, or pass --devcontainer-dir <path>.`,
		)
	}
	const projectDir = dirname(devcontainerDir)
	return { devcontainerDir, projectDir, envFile: join(devcontainerDir, '.env') }
}

function implicitDevcontainerDir(cwd: string): string {
	return basename(cwd) === '.devcontainer' ? cwd : join(cwd, '.devcontainer')
}

/**
 * Default `DC_PROJECT` when `.env` does not set one.
 *
 * @remarks
 * The bash script hardcoded `symptems` here (and `{{PROJECT_ID}}` in the
 * template copies) — one project's name baked into what is about to become a
 * public npm package, feeding both the Docker credentials volume name and the
 * base image tag. The directory name is the obvious non-leaky default, and it
 * is what `install.sh` prompted the user for anyway.
 *
 * Sanitised to what Docker accepts in a volume name and an image tag:
 * lowercase alphanumerics, `-`, `_`, `.`, never leading with a separator.
 */
export function defaultProjectId(projectDir: string): string {
	const sanitised = basename(projectDir)
		.toLowerCase()
		.replace(/[^a-z0-9_.-]+/g, '-')
		.replace(/^[^a-z0-9]+/, '')
		.replace(/-+$/, '')
	return sanitised.length > 0 ? sanitised : 'devcontainer'
}

/** Render `path` relative to `base` for log lines, mirroring `${p#$DIR/}`. */
export function relativeTo(base: string, path: string): string {
	const prefix = base.endsWith('/') ? base : `${base}/`
	return path.startsWith(prefix) ? path.slice(prefix.length) : path
}
