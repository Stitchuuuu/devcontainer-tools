// Runtime state file holding the epoch seconds of the last popup fire.
// Plaintext 8-byte big-endian u64, written atomically via .tmp+rename.
// Read by the service tick loop at cold start and by the Config UI for
// the "Prochain contrôle" live preview. Missing / corrupt = None.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

pub const PATH: &str = r"C:\ProgramData\DiagnosticsCache\runtime.dat";

pub fn read() -> Option<SystemTime> {
    let bytes = std::fs::read(PATH).ok()?;
    if bytes.len() != 8 {
        return None;
    }
    let secs = u64::from_be_bytes(bytes.try_into().ok()?);
    Some(UNIX_EPOCH + Duration::from_secs(secs))
}

pub fn write(t: SystemTime) -> Result<()> {
    let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let bytes = secs.to_be_bytes();
    let path = Path::new(PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let tmp = path.with_extension("dat.tmp");
    {
        let mut f = std::fs::File::create(&tmp)
            .with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}
