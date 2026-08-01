// Rolling N-line progress window — the port of run_with_progress
// (initialize.sh:224-294).
//
// Half of the bash implementation existed to undo a problem bash created for
// itself. Because `exec > >(tee …)` had already replaced fd 1 with a pipe, the
// function could not ask whether it was on a terminal (`[ -t 1 ]` was always
// false, hence ORIG_STDOUT_TTY captured before the redirect) and it could not
// print cursor escapes to stdout without staining the .log (hence every redraw
// going to the saved fd 3).
//
// Neither problem exists here. Nothing is redirected, so isTTY is simply true
// or false; and the window writes to its own stream, which is not the log sink,
// so "no ANSI in the .log" holds by construction rather than by discipline.

import { ESC, type Logger } from './logger.js'
import { stripAnsi, truncate } from './lines.js'
import { run, type RunOptions } from './proc.js'

export interface WindowOptions {
	size: number
	columns: number
}

/**
 * One full repaint, as a single string.
 *
 * Pure — this is where the port is actually verified, since the exact escape
 * sequence is the whole behaviour. Order matches the bash lines 272-281:
 *
 * 1. cursor up `size` rows, column 1
 * 2. one erased, dim-grey, two-space-indented, truncated row per buffered line
 * 3. one erased blank row per unused slot
 */
export function renderFrame(buffer: readonly string[], options: WindowOptions): string {
	const size = Math.max(1, options.size)
	const maxWidth = Math.max(40, options.columns - 4)
	let frame = `${ESC}[${size}F`
	for (const line of buffer.slice(0, size)) {
		frame += `${ESC}[2K${ESC}[2;37m  ${truncate(stripAnsi(line), maxWidth)}${ESC}[0m\n`
	}
	for (let i = buffer.length; i < size; i++) frame += `${ESC}[2K\n`
	return frame
}

/**
 * Erase the window and park the cursor at its top, leaving the banner above
 * intact (bash lines 289-291).
 */
export function clearFrame(size: number): string {
	const rows = Math.max(1, size)
	return `${ESC}[${rows}F${`${ESC}[2K\n`.repeat(rows)}${ESC}[${rows}F`
}

/**
 * Reserve the rows, then repaint on demand.
 *
 * Repaints are coalesced on a timer instead of firing once per input line.
 * `docker build --progress=plain` on a cold cache emits tens of thousands of
 * lines; bash repainted every row for each one, which is both wasteful and
 * visibly flickery. This is a deliberate divergence.
 */
export class RollingWindow {
	private readonly buffer: string[] = []
	private timer: NodeJS.Timeout | null = null
	private dirty = false

	constructor(
		private readonly out: NodeJS.WritableStream,
		private readonly size: number,
		private readonly throttleMs = 50,
	) {}

	open(): void {
		this.out.write('\n'.repeat(Math.max(1, this.size)))
	}

	push(line: string): void {
		if (this.buffer.length >= this.size) this.buffer.shift()
		this.buffer.push(line)
		this.dirty = true
		if (this.timer !== null) return
		this.timer = setTimeout(() => {
			this.timer = null
			this.paint()
		}, this.throttleMs)
		// A pending repaint must never be the reason the process stays alive.
		this.timer.unref?.()
	}

	close(): void {
		if (this.timer !== null) {
			clearTimeout(this.timer)
			this.timer = null
		}
		this.out.write(clearFrame(this.size))
	}

	private paint(): void {
		if (!this.dirty) return
		this.dirty = false
		this.out.write(renderFrame(this.buffer, { size: this.size, columns: currentColumns(this.out) }))
	}
}

/**
 * Re-read the width on every frame.
 *
 * Bash captured `tput cols` once, so resizing the terminal mid-build wrapped
 * every row from then on. Reading it per frame is free and handles SIGWINCH
 * without a handler.
 */
function currentColumns(out: NodeJS.WritableStream): number {
	const columns = (out as NodeJS.WriteStream).columns
	return typeof columns === 'number' && columns > 0 ? columns : 100
}

export interface ProgressOptions extends Omit<RunOptions, 'onLine' | 'check'> {
	logger: Logger
	/** Build log the full output is teed to, ANSI-free by construction. */
	logPath: string
	title: string
	size?: number
	/** Injected in tests; defaults to the logger's terminal stream. */
	out?: NodeJS.WritableStream
}

/**
 * Run a command behind a rolling window, returning its exit code.
 *
 * Never throws on a non-zero exit — the caller decides what a failed build
 * means, exactly as the bash version returned `$rc` for `build_base_if_missing`
 * to interpret.
 */
export async function runWithProgress(options: ProgressOptions): Promise<number> {
	const { logger, title, logPath } = options
	const size = options.size ?? 10
	const out = options.out ?? process.stdout

	if (!isWindowCapable(logger, out)) {
		// Plain tee — bash lines 242-247. Escapes here would render as garbage
		// in a VS Code output channel, which is the common case.
		logger.log(`▸ ${title}`)
		const plain = await run({ ...options, onLine: (line) => logger.raw(line), check: false })
		return plain.code
	}

	logger.styled('1;36', `▸ ${title}`)
	const window = new RollingWindow(out, size)
	window.open()
	try {
		const result = await run({
			...options,
			// The window paints the terminal; the log gets the same lines with
			// no escapes attached. One source, two very different renderings.
			onLine: (line) => {
				logger.rawToLogOnly(line)
				window.push(line)
			},
			check: false,
			logPath,
		})
		return result.code
	} finally {
		window.close()
	}
}

function isWindowCapable(logger: Logger, out: NodeJS.WritableStream): boolean {
	if (!logger.isTTY) return false
	const env = process.env
	if (env['NO_COLOR'] !== undefined && env['NO_COLOR'] !== '') return false
	if (env['TERM'] === 'dumb') return false
	if (env['CI'] !== undefined && env['CI'] !== '') return false
	return (out as NodeJS.WriteStream).isTTY === true
}
