# -----------------------------------------------
# Project DevContainer — slim layer over the published base image
# ghcr.io/meitogi/devcontainer-sandbox (repo: meitogi/devcontainer-sandbox).
# Compose pulls it — nothing is built locally except this thin layer.
#
# To bump Claude or the base: set BASE_IMAGE in .devcontainer/.env (tag
# scheme <base-version>-cc<claude-code-version>, published pairs listed in
# the base repo's cc-versions.json), then "Rebuild Container" in VS Code.
# The three published lines: 1.9.1-cc2.1.280 (default), 1.9.1-cc2.1.272,
# 1.9.1-cc2.1.220.
# -----------------------------------------------
ARG BASE_IMAGE=ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.280

# -----------------------------------------------
# Stage 1 — firewall bake (throwaway)
# -----------------------------------------------
# firewall/ carries domains.local.txt and policy.local.d/, which are gitignored
# and writable by anything running in the container. A `RUN rm` after a COPY
# would not help — the file still sits in the COPY layer. Layers built in a
# stage that nothing copies from never enter the final manifest, so the .local
# files are discarded rather than merely hidden. The bake also freezes the
# compiled ruleset into effective/, which boot installs verbatim.
FROM ${BASE_IMAGE} AS fw-bake

# 0 = hardened (default) : domains.local.txt and policy.local.d/ are NOT baked.
# 1 = convenience : they are. Set per developer through .env, never as a team
# default. Either way the bake logs which it did and how many hosts it costs.
ARG FIREWALL_ALLOW_LOCAL_AT_REBUILD=0

USER root
COPY firewall/ /tmp/fw-src/
RUN FIREWALL_ALLOW_LOCAL_AT_REBUILD="${FIREWALL_ALLOW_LOCAL_AT_REBUILD}" \
    /usr/local/bin/firewall-docker-setup.sh --src /tmp/fw-src --dest /out

# -----------------------------------------------
# Final image
# -----------------------------------------------
FROM ${BASE_IMAGE}

USER root

# /out already merges the base image's firewall content with the project
# overlay, so this single COPY is the whole configuration. Ownership and modes
# were set root:root / u=rwX,go=rX in the bake stage and COPY --from preserves
# them. The assertion catches a bake that silently produced nothing.
COPY --from=fw-bake /out/ /etc/devcontainer-firewall/
RUN test -s /etc/devcontainer-firewall/baked-at \
 && test -s /etc/devcontainer-firewall/effective/sources.sha256

# Project-specific layer — carried from the v2 Dockerfile as written.
# Add `RUN apt-get install -y …` here for project deps that should NOT live
# in the shared base image (see the base repo's stacks/*.md for PHP / Android /
# Capacitor blocks).

# --- Animation fixture tooling (dev-only, rarely used) ------------------
# Regenerates purrpause's raster/video test fixtures (GIF / animated WebP /
# APNG / alpha WebM) from the bundled Lottie, so every format benchmarks the
# SAME animation. Isolated in a venv to sidestep PEP-668; the driver script
# (apps/purrpause/scripts/gen-anim-fixtures.py) calls the venv python by
# absolute path, so nothing here shadows the default python3 for other work.
# ffmpeg ships libvpx-vp9 for the alpha WebM path. Purely a build-time
# convenience — none of this is embedded in the purrpause binary.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ffmpeg python3-venv \
 && rm -rf /var/lib/apt/lists/*
RUN python3 -m venv /opt/anim-tools \
 && /opt/anim-tools/bin/pip install --no-cache-dir rlottie-python pillow lottie
# -----------------------------------------------------------------------

# --- Docker CLI (client only) --------------------------------------------
# Talks to the nested dind sidecar via DOCKER_HOST=tcp://dind:2375 (see
# docker-compose.yml) so the agent can run the release-check suites against
# a daemon whose container list does not include this container. Client
# only — no engine, no socket. Deliberately NOT in the shared base image:
# it would eat into the 1200 MiB verify-slim-base cap and ships to users
# who have no dind.
# buildx is required: release-check.sh's `docker build --progress=plain`
# needs it, and its step-1 VSIX check anchors on buildkit's output prefix.
RUN install -m 0755 -d /etc/apt/keyrings \
 && curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc \
 && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" > /etc/apt/sources.list.d/docker.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends docker-ce-cli docker-compose-plugin docker-buildx-plugin \
 && rm -rf /var/lib/apt/lists/*
# -----------------------------------------------------------------------


USER node

# Rust toolchain — installed as `node` so /home/node/.cargo is user-owned.
# Minimal profile keeps the image small; rustfmt/clippy/rust-analyzer are
# needed by the notif-cli workspace built in follow-up sessions.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain stable --profile minimal \
                   --component rustfmt --component clippy --component rust-analyzer
ENV PATH="/home/node/.cargo/bin:${PATH}"

# Rust cross-compile via zig-as-linker + cargo-zigbuild. Ships the SDK stubs
# needed for macOS / Windows / Linux-musl targets without installing Xcode CLT
# or MinGW-w64. Enables cross-building the packages/notifier/ workspace from
# this aarch64 Linux container. Bump ZIG_VERSION as new stables ship.
ARG ZIG_VERSION=0.14.0
RUN ARCH="$(uname -m)" \
 && curl -fsSL -o /tmp/zig.tar.xz \
      "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${ARCH}-${ZIG_VERSION}.tar.xz" \
 && mkdir -p /home/node/.local/zig \
 && tar -xJf /tmp/zig.tar.xz -C /home/node/.local/zig --strip-components=1 \
 && rm /tmp/zig.tar.xz
ENV PATH="/home/node/.local/zig:${PATH}"

# cargo-zigbuild wraps `cargo build` and swaps the C linker for `zig cc`.
# Apple Silicon + Intel Mac targets pre-installed so `cargo zigbuild --target
# {aarch64,x86_64}-apple-darwin` works out of the box.
RUN cargo install --locked cargo-zigbuild \
 && rustup target add aarch64-apple-darwin x86_64-apple-darwin

# cargo-xwin — cross-compile to Windows MSVC from Linux, using LLVM `lld-link`
# + Microsoft's Windows SDK auto-fetched by `xwin` from aka.ms (see the
# `xwin.txt` firewall bucket). Both target archs pre-installed :
#   - aarch64-pc-windows-msvc : native for Parallels Windows 11 on Apple Silicon
#   - x86_64-pc-windows-msvc  : Intel Macs + x64 Windows VMs + Windows-11-ARM
#                                emulating x64 via Prism
# The SDK cache lives at $XDG_CACHE_HOME/cargo-xwin/ (~500 MB per arch) and is
# NOT pre-populated in the image — it is fetched on first `cargo xwin build`
# so image size stays reasonable ; a re-fetch happens after every rebuild.
RUN cargo install --locked cargo-xwin \
 && rustup target add aarch64-pc-windows-msvc x86_64-pc-windows-msvc
