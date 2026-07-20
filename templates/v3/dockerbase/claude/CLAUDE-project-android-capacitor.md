# Capacitor Android plugin — Claude rules (opt-in add-on)

> Copy sections into the project's `CLAUDE-project.md` if the repo is a
> Capacitor Android plugin. Assumes devcontainer built from
> `Dockerfile.CapacitorAndroid{Min|Std}` (kchk / kchk-matrix / fetch-android-lib
> on PATH). See "Details" section below for reasoning and edge cases.

## Quick refs

- **Plugin sources** : `android/src/main/java/**/*.{kt,java}`
- **Compile 1 file** (API 35 default) : `kchk <file>`
- **Compile matrix** (8 APIs at once) : `kchk-matrix <file...>`
- **API-specific check** : `kchk-api <N> <file>` (N ∈ baked: 23 24 26 28 31 33 34 35, or cached via fetch-android-api)
- **Fetch extra Maven dep** : `fetch-android-lib <group:artifact:version>` → cached in `.devcontainer/cache/android-jars/maven/` (persistent, survives rebuilds)
- **Delivery gate** : `bash .devcontainer/scripts/capacitor-plugin-check.sh`

## Rules

1. **Every delivery runs the gate.** Before commit / PR / marking a task
   done, run `capacitor-plugin-check.sh`. If any API fails, fix the code
   or explicitly document in the commit body why the fail is acceptable
   (e.g. plugin's `minSdk` is above the failing API).

2. **Multi-file compile.** A plugin references sibling classes across
   files. `kchk` on a single file will fail with `unresolved reference:
   MyHelper` if the sibling isn't in the arg list. Pass all files at
   once — the check script does this for you.

3. **Missing lib.** If compile fails with `unresolved reference: <pkg>`
   from an androidx / third-party lib not baked, run `fetch-android-lib
   <coord>` once. It caches persistently, so subsequent runs are
   instant.

4. **Old device support.** For a plugin targeting `minSdk = X`, the
   matrix API < X are expected to fail (API introduced later). Ignore
   those, focus on `X ≤ API ≤ targetSdk`. Explicit check :
   `kchk-api <X> <file>`.

## Details

### Why compile-only, no build

These images check *compilability against the Android framework stub +
Capacitor SDK + common AndroidX*. They do NOT build APKs / AARs, do NOT
run tests, do NOT run instrumentation. Delivery = "the code compiles
correctly against the target APIs" ; runtime validation happens
downstream (real gradle build in a full Android SDK env, real device
tests).

### What's baked

- **JDK 17** (Debian openjdk in Min variant, Azul Zulu 17 in Std)
- **Kotlin 2.4.10** compiler
- **`/opt/android-jars/api-<N>.jar`** for each baked API — the SDK
  framework stub. Default symlink `/opt/android.jar` → API 35.
- **`/opt/cap-deps/`** — Capacitor SDK + common AndroidX +
  kotlinx.coroutines, all transitives resolved by Coursier at image
  build time.

### On-demand extension via cache

- **`/workspace/.devcontainer/cache/android-jars/apis/api-<N>.jar`** —
  additional APIs fetched via `fetch-android-api <N>`.
- **`/workspace/.devcontainer/cache/android-jars/maven/*.jar`** —
  additional Maven libs fetched via `fetch-android-lib <coord>`.

Both dirs are bind-mounted from host filesystem, so entries persist
through devcontainer rebuilds.

### What's NOT supported

- **`R.layout.*` / `R.string.*` / `R.drawable.*`** — requires `aapt2`,
  not included. If the plugin references `R.*`, either stub the R
  class by hand for compile-check, or accept the unresolved reference
  and rely on the downstream gradle build.
- **`@Composable` bytecode transforms** — Compose compiler plugin not
  wired. Compose plugins compile syntactically but not with the
  compose-compiler behavior applied. Rare for Capacitor plugin dev
  (Capacitor is not Compose-based).
- **Annotation processors** (Hilt / Room / Moshi codegen) — not run.
  Their annotations are treated as unknown by kotlinc, but the code
  compiles. Actual codegen validation requires downstream gradle.
- **APK / AAB packaging, adb, emulator** — none, by design.

### Fetch-android-lib flow

Usage: `fetch-android-lib com.squareup.retrofit2:retrofit:2.11.0`

Tries default repos in order: Google Maven, Maven Central. Downloads
.aar first (extracts `classes.jar`), falls back to .jar. Writes to
`/workspace/.devcontainer/cache/android-jars/maven/<artifact>-<version>.jar`.

Override the repo with a second arg: `fetch-android-lib <coord> <repo-url>`.

Once cached, `kchk` and `kchk-matrix` auto-include it in classpath.

### capacitor-plugin-check.sh — the delivery gate

The script under `.devcontainer/scripts/` :
1. Finds all `.kt` / `.java` under `android/src/main/`
2. Feeds them all to `kchk-matrix` in one invocation
3. Reports pass/fail per API
4. Exits non-zero if any API fails

Invoke before any commit that touches plugin Android code.
