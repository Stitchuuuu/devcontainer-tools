#!/usr/bin/env node
// build-icns.js — minimal PNG → .icns wrapper for single-resolution assets.
//
// The .icns format is `icns` magic + u32-BE total size + a sequence of
// `<OSType 4B><u32-BE elem-size><data>` elements. When the data is a PNG
// blob, macOS reads the pixels directly — no need for the legacy
// ARGB / RLE tables. This tool wraps a single PNG at the OSType matching
// its resolution : ic07 (128 px), ic09 (512 px), ic10 (1024 px), etc.
//
// Usage : node build-icns.js <input.png> <output.icns> [--ostype ic07]
//
// Not a Rust module because it runs from a Node context (during
// devcontainer bootstrap) — icon-gen is SVG-only and would need to grow
// PNG input handling for this one asset. YAGNI ; wrap in JS.

const fs = require('fs')

function main() {
	const [, , inPath, outPath, ...flags] = process.argv
	if (!inPath || !outPath) {
		console.error('usage: build-icns.js <input.png> <output.icns> [--ostype ic07]')
		process.exit(2)
	}

	let osType = 'ic07'  // 128 px default — matches the Claude Code marketplace icon.
	for (let i = 0; i < flags.length; i++) {
		if (flags[i] === '--ostype' && flags[i + 1]) osType = flags[i + 1]
	}
	if (osType.length !== 4) {
		console.error(`error: --ostype must be exactly 4 ASCII chars (got ${osType})`)
		process.exit(2)
	}

	const png = fs.readFileSync(inPath)
	// PNG magic check — refuse non-PNG input up front rather than write
	// garbage into a .icns.
	if (png.length < 8 || png.readUInt32BE(0) !== 0x89504e47) {
		console.error(`error: ${inPath} is not a PNG (magic mismatch)`)
		process.exit(2)
	}

	// Element : OSType (4) + u32-BE elem-size incl. header (4) + data.
	const elemHeader = Buffer.alloc(8)
	elemHeader.write(osType, 0, 4, 'ascii')
	elemHeader.writeUInt32BE(8 + png.length, 4)
	const element = Buffer.concat([elemHeader, png])

	// File : "icns" magic (4) + u32-BE total-size incl. header (4) + elements.
	const fileHeader = Buffer.alloc(8)
	fileHeader.write('icns', 0, 4, 'ascii')
	fileHeader.writeUInt32BE(8 + element.length, 4)

	fs.writeFileSync(outPath, Buffer.concat([fileHeader, element]))
	console.log(`wrote ${outPath} — ${8 + element.length} bytes, OSType=${osType}, PNG=${png.length} B`)
}

main()
