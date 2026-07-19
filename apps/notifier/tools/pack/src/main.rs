//! Post-build packager for the notif Windows binary.
//!
//! Runs on the devcontainer host, invokes
//! `cargo zigbuild --release --target <triple> --bin notif` for each
//! requested target, then zips the resulting `notif.exe` into
//! `target/dist/notif-<version>-<arch>.zip`.
//!
//! Invoke from `apps/notifier/` :
//!
//! ```text
//! cargo run -p notif-pack --release                 # aarch64 (default)
//! cargo run -p notif-pack --release -- --aarch64    # aarch64 only
//! cargo run -p notif-pack --release -- --x64        # x64 only
//! cargo run -p notif-pack --release -- --both       # both
//! cargo run -p notif-pack --release -- --no-build   # skip zigbuild, reuse exes
//! ```

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

const EXE_NAME: &str = "notif.exe";

struct Target {
    triple: &'static str,
    arch_suffix: &'static str,
}

const AARCH64: Target = Target {
    triple: "aarch64-pc-windows-gnullvm",
    arch_suffix: "aarch64",
};

const X64: Target = Target {
    triple: "x86_64-pc-windows-gnullvm",
    arch_suffix: "x64",
};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let skip_build = args.iter().any(|a| a == "--no-build");
    let only_aarch64 = args.iter().any(|a| a == "--aarch64");
    let only_x64 = args.iter().any(|a| a == "--x64");
    let both = args.iter().any(|a| a == "--both");

    let targets: Vec<&Target> = match (only_aarch64, only_x64, both) {
        (_, _, true) => vec![&AARCH64, &X64],
        (true, false, _) => vec![&AARCH64],
        (false, true, _) => vec![&X64],
        // Default : aarch64 only (the smoke host is ARM64).
        _ => vec![&AARCH64],
    };

    let version = read_cargo_version()?;

    for target in targets {
        pack_target(target, &version, skip_build)?;
    }
    Ok(())
}

fn pack_target(target: &Target, version: &str, skip_build: bool) -> Result<()> {
    println!("[pack] notif v{version} → {}", target.triple);

    if !skip_build {
        println!(
            "[pack] cargo zigbuild --release --target {} --bin notif",
            target.triple,
        );
        let status = Command::new("cargo")
            .arg("zigbuild")
            .arg("--release")
            .arg("--target")
            .arg(target.triple)
            .arg("--bin")
            .arg("notif")
            .status()
            .context("spawn cargo zigbuild")?;
        if !status.success() {
            anyhow::bail!("cargo zigbuild failed for {} (exit {status})", target.triple);
        }
    } else {
        println!("[pack] --no-build : skipping cargo zigbuild for {}", target.triple);
    }

    let release_dir = PathBuf::from("target").join(target.triple).join("release");
    let exe = release_dir.join(EXE_NAME);
    if !exe.exists() {
        anyhow::bail!("exe not found : {}", exe.display());
    }

    let stage_name = format!("notif-{version}-{}", target.arch_suffix);
    let dist_dir = PathBuf::from("target").join("dist");
    fs::create_dir_all(&dist_dir).context("mkdir target/dist")?;

    // Wipe any prior zip for this same version + arch so a re-pack after a
    // dirty rebuild doesn't leave stale bytes.
    let zip_path = dist_dir.join(format!("{stage_name}.zip"));
    if zip_path.exists() {
        fs::remove_file(&zip_path)
            .with_context(|| format!("remove stale {}", zip_path.display()))?;
    }

    let file = fs::File::create(&zip_path)
        .with_context(|| format!("create {}", zip_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let arc_name = format!("{stage_name}/{EXE_NAME}");
    add_file(&mut zip, &exe, &arc_name, options)?;

    zip.finish().context("finalize zip")?;

    let size = fs::metadata(&zip_path)?.len();
    println!(
        "[pack] wrote {} ({:.2} MiB, 1 file)",
        zip_path.display(),
        size as f64 / 1024.0 / 1024.0,
    );
    Ok(())
}

fn read_cargo_version() -> Result<String> {
    let text = fs::read_to_string("Cargo.toml").context("read Cargo.toml")?;
    let mut in_workspace_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }
        if !in_workspace_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version") {
            if let Some(v) = rest
                .split('=')
                .nth(1)
                .and_then(|s| s.trim().strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
            {
                return Ok(v.to_string());
            }
        }
    }
    Err(anyhow!("no version key found in [workspace.package] of Cargo.toml"))
}

fn add_file(
    zip: &mut zip::ZipWriter<fs::File>,
    src: &Path,
    arc_name: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    zip.start_file(arc_name, options).context("start_file")?;
    let mut f = fs::File::open(src).with_context(|| format!("open {}", src.display()))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).context("read")?;
    zip.write_all(&buf).context("write")?;
    Ok(())
}
