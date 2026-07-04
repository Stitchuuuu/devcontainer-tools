# Vendored third-party bits

Files here are not authored by this project — they are third-party artefacts
we ship so builds are hermetic. Never treat these as maintainable code.

## `macos-sdk/`

Minimal subset of the **macOS 14.5 SDK** stub tree, sourced from the OSS
mirror at [github.com/joseluisq/macosx-sdks](https://github.com/joseluisq/macosx-sdks).
Only the frameworks + `libobjc` we actually link are checked in — see
[fetch-sdk.sh](fetch-sdk.sh) for the exact selection and re-run recipe.

**Why 14.5 specifically** — the user's Xcode 26.2 (macOS 26 SDK preview)
Foundation.tbd hits zig #24615 (open) and SIGSEGV's zig's TAPI parser at
link time on aarch64-apple-darwin and x86_64-apple-darwin. macOS 14.5
(Sonoma) stubs cover every symbol we reference from Foundation /
UserNotifications / CoreFoundation / CoreLocation with a TAPI-v4 layout
zig 0.14 handles cleanly. The produced Mach-O runs on any macOS ≥ 11.0 —
our `Info.plist` `LSMinimumSystemVersion=11.0` was set well below 14.5.

**When to bump** — when zig upstream fixes #24615 (allowing Xcode 26.x
stubs) *or* when we need a symbol added after 14.5 (SDK 15.x / 16.x has
newer AppKit / UserNotifications additions we don't currently use). Both
are noise items; today's tree stays 14.5 until a real need surfaces.
