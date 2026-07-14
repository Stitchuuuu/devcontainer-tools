// Sidecar UAC manifest deployment.
//
// The `embed-manifest` crate emits linker flags (`/MANIFEST:EMBED`
// `/MANIFESTINPUT:...`) that require Microsoft's `mt.exe`, which is not
// shipped by xwin, llvm-tools-preview, or any tool in this devcontainer.
// Instead we ship `SystemHealthAgent.exe.manifest` next to the exe :
// Windows loads a sidecar manifest automatically at process creation if
// `<exe>.manifest` exists in the same directory. Same UAC behavior as an
// embedded manifest, zero external tooling.
//
// This build script copies the source manifest to the binary output
// directory so `cargo build` / `cargo xwin build` produce a ready-to-ship
// pair (SystemHealthAgent.exe + SystemHealthAgent.exe.manifest).
use std::path::PathBuf;

fn main() {
    let manifest_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("SystemHealthAgent.exe.manifest");

    println!("cargo:rerun-if-changed=resources/SystemHealthAgent.exe.manifest");

    // OUT_DIR is `target/<triple>/<profile>/build/<crate>-<hash>/out`.
    // Walk up four levels to reach `target/<triple>/<profile>/` where the
    // exe lands after linking.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR unset");
    let target_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3)
        .expect("OUT_DIR shallower than expected")
        .to_path_buf();

    let manifest_dst = target_dir.join("SystemHealthAgent.exe.manifest");

    if let Err(e) = std::fs::copy(&manifest_src, &manifest_dst) {
        // Non-fatal : sidecar is a Windows-only UX feature, and other
        // targets (linux cargo test) don't need it. Warn only.
        println!(
            "cargo:warning=failed to deploy sidecar manifest to {}: {e}",
            manifest_dst.display()
        );
    }
}
