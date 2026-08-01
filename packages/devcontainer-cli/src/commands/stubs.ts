// Registered-but-unimplemented commands.
//
// They exist so the dispatcher, `--help` and the exit codes are exercised from
// day one, and so a user who reaches for a documented command gets told when it
// arrives instead of "unknown command". Everything else from the design §7
// table is genuinely absent until its session builds it.
//
// `devc knowledge *` and `devc lessons *` are not here and will not be: the
// rollout dropped them as YAGNI for v1 — those files are edited directly.

export interface StubCommand {
	name: string
	summary: string
	arrivesIn: string
}

export const STUB_COMMANDS: readonly StubCommand[] = [
	{ name: 'init', summary: 'Scaffold a new .devcontainer (wizard)', arrivesIn: 'session 4 — cli-init-wizard' },
	{ name: 'update', summary: 'Bump base + Claude Code versions', arrivesIn: 'session 5 — image-ghcr' },
	{ name: 'doctor', summary: 'Diagnose versions, config and warnings', arrivesIn: 'session 5 — image-ghcr' },
]

/** Non-zero: the command was recognised, but it did not do anything. */
export function runStub(stub: StubCommand): number {
	process.stderr.write(`devc ${stub.name}: not implemented yet — arrives in ${stub.arrivesIn}.\n`)
	return 1
}
