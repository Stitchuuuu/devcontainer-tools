# -----------------------------------------------
# Project DevContainer — PHP variant.
# Slim layer on top of claude-devcontainer-base + PHP 8.2 + Composer.
# Used by PHP projects via docker-compose :
#   build.dockerfile: Dockerfile.php
# Layer dedup is implicit : Docker reuses the PHP install layer across
# any project that builds with the same FROM + same RUN.
# -----------------------------------------------
ARG CLAUDE_CODE_VERSION=2.1.145
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}

USER root

# PHP 8.2 + 13 extensions covering the common Laravel / Symfony /
# CodeIgniter footprint (cli, fpm, curl, gd, mbstring, xml, zip, soap,
# intl, mysql, readline, bcmath, sockets, phar). Debian bookworm-slim
# (base image upstream) ships php8.2-* in main — no PPA required.
# Drop unused extensions inline per-project rather than trimming the
# shared layer here.
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
