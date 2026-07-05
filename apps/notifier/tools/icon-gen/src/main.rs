//! `notif-icon-gen` — rasterize an SVG master into `.icns` (macOS) +
//! `.ico` (Windows / favicon) + optional PNG previews.
//!
//! Pure-Rust pipeline: `usvg` parses, `resvg` rasterizes onto a
//! `tiny_skia::Pixmap`, we encode each pixmap to PNG and pack them into the
//! respective container binary formats by hand (both formats wrap PNG blobs
//! with a small header — no third-party encoder needed).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// Icon sizes bundled into the `.icns` output, paired with the modern
/// PNG-based OSTypes macOS looks up in the resource table.
///
/// Each logical size ships as a pair : `@1x` (icp4/icp5/ic07-ic10) for
/// non-Retina rendering + `@2x` (ic11-ic14) for Retina displays. Pixels are
/// shared across the pair at each pixel-resolution (e.g. 32 px is both the
/// `@2x` render of 16pt and the `@1x` render of 32pt) — same PNG bytes,
/// different OSType label, macOS picks the right one per display class.
///
/// Older bitmap-only OSTypes (`is32`, `il32`, …) are intentionally omitted —
/// every macOS 10.7+ deployment reads the PNG variants and Icon Composer /
/// `iconutil` haven't emitted the legacy forms since Yosemite.
const ICNS_ENTRIES: &[(u32, &[u8; 4])] = &[
    (16, b"icp4"),   // 16pt @1x
    (32, b"ic11"),   // 16pt @2x  (Retina)
    (32, b"icp5"),   // 32pt @1x
    (64, b"ic12"),   // 32pt @2x  (Retina)
    (128, b"ic07"),  // 128pt @1x
    (256, b"ic13"),  // 128pt @2x (Retina)
    (256, b"ic08"),  // 256pt @1x
    (512, b"ic14"),  // 256pt @2x (Retina)
    (512, b"ic09"),  // 512pt @1x
    (1024, b"ic10"), // 512pt @2x (Retina)
];

/// Icon sizes bundled into the `.ico` output. Windows Vista+ accepts PNG
/// blobs directly (via the `size=0` = 256 encoding); pre-Vista support isn't
/// a target for a modern favicon.
const ICO_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256];

#[derive(Parser)]
#[command(name = "notif-icon-gen", version, about)]
struct Cli {
    /// Path to the source SVG (1024×1024 viewBox recommended).
    input: PathBuf,
    /// Path to write the `.icns` output. Required.
    #[arg(long)]
    icns: PathBuf,
    /// Optional path to write the `.ico` output (Windows / favicon).
    #[arg(long)]
    ico: Option<PathBuf>,
    /// Optional directory to write per-size PNG previews. Created if absent.
    #[arg(long)]
    png_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let svg_bytes = fs::read(&cli.input)
        .with_context(|| format!("read SVG {}", cli.input.display()))?;
    let tree = Tree::from_data(&svg_bytes, &Options::default())
        .context("parse SVG (usvg)")?;
    let src_size = tree.size();
    let src_w = src_size.width();
    let src_h = src_size.height();

    // Union of every size we might need — dedup so we only rasterize once.
    let mut sizes: Vec<u32> = ICNS_ENTRIES
        .iter()
        .map(|(s, _)| *s)
        .chain(ICO_SIZES.iter().copied())
        .collect();
    sizes.sort_unstable();
    sizes.dedup();

    let mut pngs: std::collections::BTreeMap<u32, Vec<u8>> =
        std::collections::BTreeMap::new();
    for &size in &sizes {
        let png = rasterize_png(&tree, size, src_w, src_h)
            .with_context(|| format!("rasterize {size}px"))?;
        pngs.insert(size, png);
    }

    if let Some(dir) = &cli.png_dir {
        fs::create_dir_all(dir)
            .with_context(|| format!("mkdir -p {}", dir.display()))?;
        for (size, bytes) in &pngs {
            let path = dir.join(format!("notify-{size}.png"));
            fs::write(&path, bytes)
                .with_context(|| format!("write {}", path.display()))?;
            eprintln!("wrote {} ({} B)", path.display(), bytes.len());
        }
    }

    let icns = build_icns(&pngs);
    fs::write(&cli.icns, &icns)
        .with_context(|| format!("write {}", cli.icns.display()))?;
    eprintln!("wrote {} ({} B, {} sizes)", cli.icns.display(), icns.len(), ICNS_ENTRIES.len());

    if let Some(ico_path) = &cli.ico {
        let ico = build_ico(&pngs)?;
        fs::write(ico_path, &ico)
            .with_context(|| format!("write {}", ico_path.display()))?;
        eprintln!("wrote {} ({} B, {} sizes)", ico_path.display(), ico.len(), ICO_SIZES.len());
    }

    Ok(())
}

/// Rasterize `tree` into a square `size`×`size` PNG. Applies a uniform scale
/// from the SVG's viewBox to fit — assumes a roughly square input.
fn rasterize_png(tree: &Tree, size: u32, src_w: f32, src_h: f32) -> Result<Vec<u8>> {
    let mut pixmap =
        Pixmap::new(size, size).context("allocate pixmap (out of memory?)")?;
    let sx = size as f32 / src_w;
    let sy = size as f32 / src_h;
    resvg::render(tree, Transform::from_scale(sx, sy), &mut pixmap.as_mut());
    pixmap.encode_png().context("encode PNG")
}

/// Pack the PNG blobs into a `.icns` container.
///
/// Format : `'icns' <big-endian u32 total-len>` header followed by a stream
/// of `<OSType 4B><big-endian u32 elem-len><PNG bytes>` elements. The
/// `elem-len` field is inclusive of the 8-byte header.
fn build_icns(pngs: &std::collections::BTreeMap<u32, Vec<u8>>) -> Vec<u8> {
    let mut body = Vec::new();
    for (size, ostype) in ICNS_ENTRIES {
        let png = &pngs[size];
        let elem_len = (8 + png.len()) as u32;
        body.extend_from_slice(*ostype);
        body.extend_from_slice(&elem_len.to_be_bytes());
        body.extend_from_slice(png);
    }
    let total_len = (8 + body.len()) as u32;
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&total_len.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// Pack the PNG blobs into a `.ico` container.
///
/// Format :
///   header : `<reserved u16=0><type u16=1><count u16>` (6 bytes)
///   dir    : `count` entries of 16 bytes each, indexing into the blob table
///   blobs  : each entry's PNG bytes, laid out sequentially after the dir
///
/// PNG blobs are accepted by Windows Vista and later — trivial vs the
/// legacy BMP path. Sizes ≥ 256 use the `0` sentinel in the width/height
/// fields per the format spec.
fn build_ico(pngs: &std::collections::BTreeMap<u32, Vec<u8>>) -> Result<Vec<u8>> {
    let count = ICO_SIZES.len();
    let header_len = 6 + count * 16;
    let mut offsets = Vec::with_capacity(count);
    let mut running = header_len as u32;
    for size in ICO_SIZES {
        offsets.push(running);
        running += pngs[size].len() as u32;
    }

    let mut out = Vec::with_capacity(running as usize);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(count as u16).to_le_bytes());

    for (i, size) in ICO_SIZES.iter().enumerate() {
        let png = &pngs[size];
        let dim: u8 = if *size >= 256 { 0 } else { *size as u8 };
        out.push(dim);
        out.push(dim);
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offsets[i].to_le_bytes());
    }

    for size in ICO_SIZES {
        out.extend_from_slice(&pngs[size]);
    }

    Ok(out)
}
