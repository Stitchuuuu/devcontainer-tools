# Android — Project rules add-on (opt-in, manual)

> This file is **not auto-loaded**. Copy the sections you need into
> the project's `CLAUDE-project.md` when the repo does Android /
> Kotlin work. Companion to `templates/v2/firewall/domains.android.txt`
> (network allowlist) and `Dockerfile.AndroidMin` /
> `Dockerfile.AndroidStd` (compile-check images).

## Compile / check tooling

The template provides a container image with `kchk` (Kotlin) and `jchk`
(Java) helpers that compile source against the `android.jar` stub
(API 35). The concrete image (Alpine / vendor JDK / etc.) is a project
choice — check the project's `docker-compose.yml` or equivalent for the
tag actually used. What the rules below assume is only that :

- The image exposes `kchk` and `jchk` binaries on PATH.
- `$ANDROID_JAR` points to the framework stub inside the image.
- The image does **not** include AndroidX / Compose / build-tools /
  gradle / adb — those are added per project via the fetch flow below.

## Cache path — MUST use `.devcontainer/cache/`

> **All Android-related caches (jars, AAR-extracted classes, downloaded
> tools) land in `.devcontainer/cache/android-jars/`.** No other path.

Reasons :

- `.devcontainer/` is bind-mounted from host → survives every container
  rebuild / stop / recreate.
- Any other path (`/tmp`, `$HOME` inside container, etc.) is on the
  ephemeral container FS → wiped on rebuild, download effort lost.
- Precedent already exists : `.devcontainer/pending/` is gitignored
  the same way (`.devcontainer/.gitignore` has `pending/*`).

Gitignore requirement — add to `.devcontainer/.gitignore` :

    cache/

Cache structure follows Maven layout for easy `-cp` reuse :

    .devcontainer/cache/android-jars/
    ├── androidx/compose/material3/material3/1.3.1/classes.jar
    ├── androidx/compose/ui/ui/1.7.5/classes.jar
    ├── org/jetbrains/kotlinx/kotlinx-coroutines-core/1.9.0/kotlinx-coroutines-core-1.9.0.jar
    └── ...

## Fetch flow (when a `.kt` needs a lib not in `android.jar`)

1. Detect the missing coordinate from the compile error (`unresolved
   reference: androidx.compose.material3`).
2. Curl the artifact from the right Maven repo :
   - AndroidX / Compose / Google → `https://dl.google.com/android/maven2/…`
   - Kotlin / kotlinx / third-party → `https://repo.maven.apache.org/maven2/…`
   URL from coordinate : `group.replace('.','/') / artifact / version / artifact-version.{jar|aar}`
3. If `.aar` (Android libs), extract `classes.jar` :
   `unzip -p my-lib.aar classes.jar > my-lib.jar`
4. Store under `.devcontainer/cache/android-jars/<group-path>/<artifact>/<version>/`.
5. Extend `kchk` classpath at run time :

       docker run --rm \
         -v "$PWD:/src" \
         -v "$PWD/.devcontainer/cache/android-jars:/deps:ro" \
         android-min sh -c 'kotlinc -cp "$ANDROID_JAR:$(find /deps -name "*.jar" -printf "%p:")" \
           -d /tmp/out.jar MyScreen.kt'

Prerequisites — the network hosts must be allowlisted in the current
devcontainer firewall (mode `basic` : host-level allow only). See
`templates/v2/firewall/domains.android.txt` and copy the "Level B —
Maven / Gradle package repos" block into `domains.local.txt` (personal)
or `domains.txt` (team). Rebuild devcontainer once after the edit —
mid-session edits don't refresh the running firewall.

## Limitations to warn the user about

- **No AndroidX / Compose out-of-box.** Compile errors on
  `androidx.*` imports trigger the fetch flow above. First fetch per
  version is slow (~5s per jar), subsequent runs are cache hits.
- **`@Composable`** — requires the Compose compiler plugin
  (`org.jetbrains.kotlin:kotlin-compose-compiler-plugin-embeddable`)
  passed via `-Xplugin=/deps/.../compose-compiler-plugin.jar`.
  Without it, code compiles but @Composable functions get "unknown
  annotation" warnings and bytecode transforms don't apply. Fine for
  syntax check, wrong for actual runtime bytecode.
- **Annotation processors** (Hilt / Room / Moshi / Dagger) — NOT run.
  Their annotations get "unknown" warnings. Types still resolve if
  the lib jars are on the classpath.
- **`R.layout.*` / `R.string.*`** — impossible without `aapt2` (build
  tools). Workaround for dev-only compile check : hand-write a stub
  `R.kt` with the IDs referenced by the file under test.
- **Cannot RUN the code.** No emulator, no adb, no APK build. These
  images are compile-check only.

## Recommended kchk-ext pattern (when helper script ships)

    fetch-android-deps <project>/deps.txt   # curl missing jars into cache
    kchk-ext <project> MyFile.kt             # kchk + auto /deps mount + full classpath

`deps.txt` format (one Maven coordinate per line) :

    androidx.compose.runtime:runtime:1.7.5
    androidx.compose.ui:ui:1.7.5
    androidx.compose.material3:material3:1.3.1
    org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0
