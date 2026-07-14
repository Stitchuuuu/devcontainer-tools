// Manifest embedding is deferred to a later session.
//
// The `embed-manifest` crate emits linker flags (`/MANIFEST:EMBED`
// `/MANIFESTINPUT:...`) that require Microsoft's `mt.exe`, which is not
// shipped by xwin, llvm-tools-preview, or any tool in this devcontainer.
// Session 2 (install flow) will either :
//   - handle UAC elevation via the installer script (`runas`
//     invocation from a wrapper), skipping the embedded requestedExecutionLevel
//   - or embed the manifest at a Windows-native post-build step
//
// For session 1 (scaffold), we ship without the manifest to keep the
// cross-compile pipeline linear. The exe still runs — it just doesn't
// trigger UAC on double-click.
fn main() {}
