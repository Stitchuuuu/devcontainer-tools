# -----------------------------------------------
# Project DevContainer — PHP variant.
# Slim layer on top of claude-devcontainer-base + PHP 8.2 + Composer.
# Used by PHP projects via docker-compose :
#   build.dockerfile: Dockerfile.php
# Layer dedup is implicit : Docker reuses the PHP install layer across
# any project that builds with the same FROM + same RUN.
# -----------------------------------------------
ARG CLAUDE_CODE_VERSION=2.1.145
ARG DC_PROJECT={{PROJECT_ID}}
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}-${DC_PROJECT}

USER root

# Sury APT repo — Ondřej Surý (official Debian PHP maintainer). Future-proofing
# for when node:24-slim eventually rebases on Debian trixie (where main ships
# PHP 8.4 by default, no php8.2-* package). On the current bookworm-based
# node:24-slim, bookworm main also ships php8.2-* — Sury wins on resolution
# because it carries the newer patch (e.g. 8.2.31 vs bookworm's 8.2.x at
# release time), which is what we want for security fixes. Codename is
# resolved at build via `lsb_release -sc`, so the same RUN block works on
# both bookworm and a future trixie base without edit.
RUN apt-get update && apt-get install -y --no-install-recommends \
      apt-transport-https lsb-release ca-certificates curl \
    && curl -fsSL https://packages.sury.org/php/apt.gpg \
        -o /usr/share/keyrings/sury-php-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/sury-php-archive-keyring.gpg] https://packages.sury.org/php/ $(lsb_release -sc) main" \
        > /etc/apt/sources.list.d/sury-php.list \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# PHP 8.2 + 13 extensions covering the common Laravel / Symfony /
# CodeIgniter footprint (cli, fpm, curl, gd, mbstring, xml, zip, soap,
# intl, mysql, readline, bcmath, sockets, phar). Package names match
# Debian's across bookworm + trixie, so the install block below is
# distro-agnostic — Sury (wired above) provides the resolution path
# on either base. Drop unused extensions inline per-project rather
# than trimming the shared layer here.
RUN apt-get update && apt-get install -y --no-install-recommends \
      php8.2-cli \
      php8.2-fpm \
      php8.2-curl \
      php8.2-gd \
      php8.2-mbstring \
      php8.2-xml \
      php8.2-zip \
      php8.2-soap \
      php8.2-intl \
      php8.2-mysql \
      php8.2-readline \
      php8.2-bcmath \
      php8.2-sockets \
      php8.2-phar \
    && apt-get clean && rm -rf /var/lib/apt/lists/* \
    && rm -rf /usr/share/doc/* /usr/share/man/*

# Composer 2.x latest (official image, multi-arch amd64 + arm64).
COPY --from=composer:2 /usr/bin/composer /usr/bin/composer

# Project-specific firewall data — baked into the image, no runtime bind mount.
# Recursive COPY embarks the whole firewall/ tree (domains, policy.d/,
# policy.local.d/, default-mode, direct-tcp-allow.txt, addons, dnsmasq.conf,
# tests, …). Base-layer COPYs into /etc/devcontainer-firewall/ from
# Dockerfile.base are overlaid by this COPY where paths overlap — projects
# can override addons/, dnsmasq.conf, tests/ that way. firewall-docker-setup.sh
# (baked in the base image) finalizes perms + chown.
COPY firewall/ /etc/devcontainer-firewall/
RUN /usr/local/bin/firewall-docker-setup.sh

USER node
