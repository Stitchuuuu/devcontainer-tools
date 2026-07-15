// Post-build packaging tool. Runs on the host (Linux devcontainer),
// invokes `cargo xwin build --release --target aarch64-pc-windows-msvc`
// then bundles the resulting `SystemHealthAgent.exe` + sidecar
// `.manifest` into `target/dist/SystemHealthAgent-<version>-aarch64.zip`.
//
// Since the assets are all embedded now (session 6, 0.5.5+), the zip
// only ships 2 files : exe + manifest. Nothing else.
//
// Invoke : `cargo run --bin pack --release`
// Skip the cross-compile step : `cargo run --bin pack --release -- --no-build`

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

const TARGET: &str = "aarch64-pc-windows-msvc";
const EXE_NAME: &str = "SystemHealthAgent.exe";
const MANIFEST_NAME: &str = "SystemHealthAgent.exe.manifest";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let skip_build = args.iter().any(|a| a == "--no-build");

    // Read version from Cargo.toml so the zip name always matches.
    let version = read_cargo_version()?;
    println!("[pack] purrpause v{version} → {TARGET}");

    if !skip_build {
        println!("[pack] cargo xwin build --release --target {TARGET}");
        let status = Command::new("cargo")
            .arg("xwin")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(TARGET)
            .status()
            .context("spawn cargo xwin build")?;
        if !status.success() {
            anyhow::bail!("cargo xwin build failed (exit {status})");
        }
    } else {
        println!("[pack] --no-build : skipping cargo xwin build");
    }

    let release_dir = PathBuf::from("target").join(TARGET).join("release");
    let exe = release_dir.join(EXE_NAME);
    let manifest = release_dir.join(MANIFEST_NAME);
    if !exe.exists() {
        anyhow::bail!("exe not found : {}", exe.display());
    }
    if !manifest.exists() {
        anyhow::bail!("manifest not found : {}", manifest.display());
    }

    let stage_name = format!("SystemHealthAgent-{version}-aarch64");
    let dist_dir = PathBuf::from("target").join("dist");
    fs::create_dir_all(&dist_dir).context("mkdir target/dist")?;

    // Remove any prior zip for this version so stale bytes don't leak
    // into the release. Wildcard cleanup at 0.5.x avoids the manual
    // `rm -rf target/dist/SystemHealthAgent-0.5.*` shell dance.
    clean_previous_zips(&dist_dir, "SystemHealthAgent-0.5.")?;
    clean_previous_zips(&dist_dir, "SystemHealthAgent-0.6.")?;

    let zip_path = dist_dir.join(format!("{stage_name}.zip"));
    let file = fs::File::create(&zip_path)
        .with_context(|| format!("create {}", zip_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Core artefacts (exe + sidecar manifest).
    let mut count = 0usize;
    for src in [&exe, &manifest] {
        let arc_name = format!(
            "{stage_name}/{}",
            src.file_name().and_then(|s| s.to_str()).unwrap()
        );
        add_file(&mut zip, src, &arc_name, options)?;
        count += 1;
    }

    // Optional convenience .bat scripts (Activer/Desactiver + Nettoyer).
    // Silently skipped if missing so the pack still succeeds for a
    // scripts-less checkout.
    let scripts_dir = PathBuf::from("scripts");
    if scripts_dir.is_dir() {
        for entry in fs::read_dir(&scripts_dir).context("read scripts/")? {
            let entry = entry.context("scripts/ entry")?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("bat") {
                let arc_name = format!(
                    "{stage_name}/{}",
                    path.file_name().and_then(|s| s.to_str()).unwrap()
                );
                add_file(&mut zip, &path, &arc_name, options)?;
                count += 1;
            }
        }
    }

    zip.finish().context("finalize zip")?;

    let size = fs::metadata(&zip_path)?.len();
    println!(
        "[pack] wrote {} ({:.2} MiB, {count} files)",
        zip_path.display(),
        size as f64 / 1024.0 / 1024.0
    );
    Ok(())
}

fn read_cargo_version() -> Result<String> {
    let text = fs::read_to_string("Cargo.toml").context("read Cargo.toml")?;
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("version") {
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
    Err(anyhow!("no version key found in Cargo.toml"))
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

fn clean_previous_zips(dist_dir: &Path, prefix: &str) -> Result<()> {
    let entries = match fs::read_dir(dist_dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(prefix) && name.ends_with(".zip") {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}
