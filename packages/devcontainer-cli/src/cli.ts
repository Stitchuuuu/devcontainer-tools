// Argument parsing and subcommand dispatch.
//
// Hand-rolled rather than pulled from a library: the surface is one real
// command with two flags, and a zero-dependency package is easier to audit and
// faster to `npx` than one that fetches an argument parser to read `--dry-run`.
//
// Exit codes:
//   0  success
//   1  the command ran and failed (or is a stub)
//   2  usage error — unknown command, unknown flag, missing flag value

import { initialize, INITIALIZE_HELP } from './commands/initialize.js'
import { runStub, STUB_COMMANDS } from './commands/stubs.js'
import { installFailureHandlers, Logger } from './lib/logger.js'
import { PathResolutionError } from './lib/paths.js'

/** A logger with no file sink — formats failures identically, writes no file. */
function terminalLogger(): Logger {
	return Logger.create({ logFile: '', silentSink: true })
}

export const EXIT_OK = 0
export const EXIT_FAILURE = 1
export const EXIT_USAGE = 2

const VERSION = '0.1.0'

const HELP = `devc — devcontainer control plane (@stitchu/devcontainer-cli v${VERSION})

Usage:
  devc <command> [options]

Commands:
  initialize                 Host-side pre-container setup (initializeCommand)
${STUB_COMMANDS.map((stub) => `  ${stub.name.padEnd(26)} ${stub.summary} [${stub.arrivesIn}]`).join('\n')}

Options:
  -h, --help                 Show this help
  -v, --version              Print the version

Run "devc <command> --help" for command-specific options.
`

export async function main(argv: readonly string[]): Promise<number> {
	// The other half of the ERR-trap analogue. `run()` defaulting to
	// `check: true` reproduces `set -e` for child processes; this catches what
	// escapes that — a throw from anywhere else, and the forgotten `await` that
	// would otherwise surface as a silent unhandled rejection.
	installFailureHandlers(terminalLogger())

	const first = argv[0]

	if (first === undefined || first === '--help' || first === '-h' || first === 'help') {
		process.stdout.write(HELP)
		return EXIT_OK
	}
	if (first === '--version' || first === '-v') {
		process.stdout.write(`${VERSION}\n`)
		return EXIT_OK
	}

	const rest = argv.slice(1)

	if (first === 'initialize') return runInitialize(rest)

	const stub = STUB_COMMANDS.find((candidate) => candidate.name === first)
	if (stub !== undefined) {
		if (rest.includes('--help') || rest.includes('-h')) {
			process.stdout.write(`devc ${stub.name} — ${stub.summary}\n\nNot implemented yet: arrives in ${stub.arrivesIn}.\n`)
			return EXIT_OK
		}
		return runStub(stub)
	}

	process.stderr.write(`devc: unknown command "${first}"\n\n${HELP}`)
	return EXIT_USAGE
}

async function runInitialize(args: readonly string[]): Promise<number> {
	let devcontainerDir: string | undefined
	let dryRun = false

	for (let i = 0; i < args.length; i++) {
		const arg = args[i] as string
		if (arg === '--help' || arg === '-h') {
			process.stdout.write(INITIALIZE_HELP)
			return EXIT_OK
		}
		if (arg === '--dry-run') {
			dryRun = true
			continue
		}
		if (arg === '--devcontainer-dir') {
			const value = args[i + 1]
			if (value === undefined || value.startsWith('-')) {
				process.stderr.write('devc initialize: --devcontainer-dir requires a path\n')
				return EXIT_USAGE
			}
			devcontainerDir = value
			i++
			continue
		}
		if (arg.startsWith('--devcontainer-dir=')) {
			devcontainerDir = arg.slice('--devcontainer-dir='.length)
			continue
		}
		process.stderr.write(`devc initialize: unknown option "${arg}"\n\n${INITIALIZE_HELP}`)
		return EXIT_USAGE
	}

	try {
		return await initialize({ devcontainerDir, dryRun, cwd: process.cwd() })
	} catch (error) {
		if (error instanceof PathResolutionError) {
			process.stderr.write(`devc initialize: ${error.message}\n`)
			return EXIT_USAGE
		}
		// The log is already closed by now, so report to the terminal.
		terminalLogger().fail(error)
		return EXIT_FAILURE
	}
}
