// The rolling window IS its escape sequence, so the assertions are on the
// exact bytes. These are the tests that prove the port, since nothing else
// about run_with_progress is observable without a real terminal.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { PassThrough } from 'node:stream'
import { ESC, Logger, type LineSink } from '../src/lib/logger.js'
import { clearFrame, renderFrame, runWithProgress } from '../src/lib/progress.js'

test('renderFrame moves up, then paints each row erased and dimmed', () => {
	const frame = renderFrame(['one', 'two'], { size: 3, columns: 100 })
	assert.equal(
		frame,
		`${ESC}[3F` +
			`${ESC}[2K${ESC}[2;37m  one${ESC}[0m\n` +
			`${ESC}[2K${ESC}[2;37m  two${ESC}[0m\n` +
			`${ESC}[2K\n`,
	)
})

test('renderFrame pads every unused slot so stale rows cannot survive', () => {
	const frame = renderFrame([], { size: 2, columns: 100 })
	assert.equal(frame, `${ESC}[2F${ESC}[2K\n${ESC}[2K\n`)
})

test('renderFrame never emits a zero-row cursor move', () => {
	// `\x1b[0F` is read as `1F` by most terminals and would eat the banner.
	assert.ok(renderFrame([], { size: 0, columns: 100 }).startsWith(`${ESC}[1F`))
})

test('renderFrame truncates to the terminal width', () => {
	const frame = renderFrame(['x'.repeat(200)], { size: 1, columns: 50 })
	assert.ok(frame.includes(`  ${'x'.repeat(46)}${ESC}[0m`), 'columns - 4 characters kept')
})

test('renderFrame enforces a 40-column floor on a narrow terminal', () => {
	const frame = renderFrame(['y'.repeat(200)], { size: 1, columns: 10 })
	assert.ok(frame.includes(`  ${'y'.repeat(40)}${ESC}[0m`))
})

test('renderFrame strips escapes emitted by the child', () => {
	const frame = renderFrame([`${ESC}[31mred${ESC}[0m`], { size: 1, columns: 100 })
	assert.equal(frame, `${ESC}[1F${ESC}[2K${ESC}[2;37m  red${ESC}[0m\n`)
})

test('clearFrame erases the window and parks the cursor at its top', () => {
	assert.equal(clearFrame(2), `${ESC}[2F${ESC}[2K\n${ESC}[2K\n${ESC}[2F`)
})

/** In-memory log sink, so a test can assert on what reached the .log file. */
function memorySink(): LineSink & { text(): string } {
	let buffer = ''
	return {
		write(text) {
			buffer += text
		},
		close() {},
		text: () => buffer,
	}
}

test('non-TTY falls back to a plain tee and emits zero escapes', async () => {
	const out = new PassThrough()
	let terminal = ''
	out.on('data', (chunk: Buffer) => {
		terminal += chunk.toString('utf8')
	})
	const sink = memorySink()
	const logger = Logger.create({
		logFile: 'unused',
		isTTY: false,
		out,
		openSink: () => sink,
	})

	const code = await runWithProgress({
		logger,
		logPath: 'unused',
		title: 'building',
		argv: [process.execPath, '-e', "process.stdout.write('line one\\nline two\\n')"],
		out,
	})

	assert.equal(code, 0)
	assert.match(terminal, /▸ building/)
	assert.match(terminal, /line one/)
	assert.doesNotMatch(terminal, new RegExp(ESC), 'no escapes on a non-TTY')
	assert.doesNotMatch(sink.text(), new RegExp(ESC), 'no escapes in the log')
})

test('the log sink never receives cursor escapes, by construction', async () => {
	// The window writes to `out`; the sink is a different object it has no
	// reference to. This is the property bash needed fd 3 to fake.
	const windowOut = new PassThrough() as unknown as NodeJS.WriteStream
	windowOut.isTTY = true
	let painted = ''
	windowOut.on('data', (chunk: Buffer) => {
		painted += chunk.toString('utf8')
	})
	const sink = memorySink()
	const logger = Logger.create({ logFile: 'unused', isTTY: true, out: windowOut, openSink: () => sink })

	const code = await runWithProgress({
		logger,
		logPath: 'unused',
		title: 'building',
		argv: [process.execPath, '-e', "process.stdout.write('compiling\\n')"],
		out: windowOut,
		size: 3,
	})

	assert.equal(code, 0)
	assert.ok(painted.includes(ESC), 'the terminal did get escapes')
	assert.doesNotMatch(sink.text(), new RegExp(ESC), 'the log did not')
	assert.match(sink.text(), /compiling/, 'but it did get the output')
})

test('a failing child returns its exit code rather than throwing', async () => {
	const out = new PassThrough()
	const logger = Logger.create({ logFile: 'unused', isTTY: false, out, openSink: () => memorySink() })
	const code = await runWithProgress({
		logger,
		logPath: 'unused',
		title: 'failing',
		argv: [process.execPath, '-e', 'process.exit(3)'],
		out,
	})
	assert.equal(code, 3)
})
