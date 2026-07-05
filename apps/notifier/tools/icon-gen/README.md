# notif-icon-gen

Dev tool that rasterizes an SVG master into `.icns` (macOS) + `.ico`
(Windows / favicon) + optional PNG previews.

Pure Rust — `usvg` parses, `resvg` rasterizes onto a `tiny_skia::Pixmap`,
we encode each pixmap to PNG (via `tiny-skia`) and pack them into the
respective container binary formats by hand (both formats wrap PNG blobs
with a small header — no third-party encoder needed).

## Usage

From the workspace root (`apps/notifier`) :

```bash
cargo run -p notif-icon-gen -- \
    crates/notif-macos/assets/notify.svg \
    --icns crates/notif-macos/assets/notify.icns \
    --ico  tools/icon-gen/preview/notify.ico \
    --png-dir tools/icon-gen/preview/
```

The **`.icns` at `crates/notif-macos/assets/notify.icns` is committed and
embedded into the binary via `include_bytes!`** at
[`crates/notif-macos/src/bundle.rs`](../../crates/notif-macos/src/bundle.rs).
Re-run this tool after touching `notify.svg` to refresh the embedded
asset.

The `.ico` output is not consumed by any crate yet — it exists as a
head-start for session 9 (Windows backend) and a favicon for any docs
site that ships later. Committing it is optional.

## Sizes emitted

- **`.icns`** — ten PNG-based OSTypes covering `@1x` + `@2x` (Retina) for
  every logical size Apple lists in the modern icon set :

  | OSType | Pixels | Logical size |
  |---|---|---|
  | `icp4` | 16 | 16pt @1x |
  | `ic11` | 32 | 16pt @2x |
  | `icp5` | 32 | 32pt @1x |
  | `ic12` | 64 | 32pt @2x |
  | `ic07` | 128 | 128pt @1x |
  | `ic13` | 256 | 128pt @2x |
  | `ic08` | 256 | 256pt @1x |
  | `ic14` | 512 | 256pt @2x |
  | `ic09` | 512 | 512pt @1x |
  | `ic10` | 1024 | 512pt @2x |

  Pixel-identical OSType pairs (`ic11`/`icp5`, `ic13`/`ic08`, `ic14`/`ic09`,
  `ic10`/would-be-`ic09@2x`) share the same PNG rasterization — the tool
  rasterizes each unique pixel resolution once and packs the bytes under
  each OSType that references it. Legacy bitmap OSTypes (`is32`, `il32`,
  `ic04`, `ic05`) are skipped ; every macOS 10.7+ deployment reads the
  PNG variants.
- **`.ico`** — 16, 32, 48, 64, 128, 256 px. PNG blobs (Vista+).
- **PNG previews** — one file per unique pixel resolution in `--png-dir`.

## Design source

`notify.svg` is the source of truth for the default icon. Palette :
`#EA692E` orange rounded-square field, `#FEFEFE` white bell-shaped `n`
glyph + clapper dot. Concept documented in
[`plans/notif-cli/scratch/icon-concepts.md`](../../../../plans/notif-cli/scratch/icon-concepts.md)
(hybrid of Direction 5 + Direction 1, decided in session 6).

Source paths are drawn to a 746×746 bounding box centered on (512, 512).
The SVG wraps them in a `<g transform="translate(512 512) scale(1.105)
translate(-512 -512)">` group so the rendered content fills ~824×824 —
the safe area recommended by Apple's HIG (Big Sur onwards, still current
as of Tahoe / macOS 26). Tahoe additionally re-masks non-squircle icons
with its own container ; our hand-drawn rounded rectangle is close enough
that the mask lines up cleanly.
