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

# Project-specific firewall data — rebuilds per-project ; does NOT affect
# the shared claude-devcontainer-base image. firewall-docker-setup.sh
# lives in the base image ; touches domains.local.txt + finalizes perms.
# *.example files are NOT COPY'd : they're host-side reference only
# (accessible from inside via /workspace/.devcontainer/firewall/ bind mount).
COPY firewall/domains.txt /etc/devcontainer-firewall/domains.txt
COPY firewall/policy.d/   /etc/devcontainer-firewall/policy.d/
RUN /usr/local/bin/firewall-docker-setup.sh

USER node
