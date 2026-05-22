# UniClaudeProxy sidecar image — bakes apt+clone+pip as cached layers so the
# container boots in <10 s after the first build (vs ~60 s with the previous
# inline bash chain on top of stock python:3.12-slim). Built and served as
# `uniclaudeproxy:local` ; see docker-compose.yml `claude-bridge` service.

FROM python:3.12-slim

# Build-time tooling : git to fetch UCP. Wipe apt lists to keep the layer thin.
RUN apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends git \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# UCP source — clones upstream default branch (currently `master`). To pin
# a specific tag/branch for reproducible builds, add `--branch <ref>` here
# and bump the image tag in docker-compose.yml.
RUN git clone --depth 1 https://github.com/vibheksoni/UniClaudeProxy /tmp/ucp \
 && cp -r /tmp/ucp/app /app/ \
 && cp /tmp/ucp/requirements.txt /app/ \
 && rm -rf /tmp/ucp

# Local overlay — Python files that override upstream UCP for the duration
# this image lives. Currently carries the `enable_thinking` flag that re-injects
# Ollama 0.9+ `reasoning` SSE deltas as <think> markers in the text stream so
# the react/_process_raw layer surfaces them as Anthropic thinking blocks.
COPY ucp-overlay/app/ /app/app/

# Wheels baked into the image — no boot-time pip install.
RUN pip install --quiet --no-cache-dir -r /app/requirements.txt

# TCP-probe healthcheck (UCP exposes only POST /v1/messages — no /health
# endpoint — so we can't do HTTP-level probing).
COPY healthcheck.sh /usr/local/bin/claude-bridge-healthcheck
RUN chmod +x /usr/local/bin/claude-bridge-healthcheck

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=3 \
  CMD /usr/local/bin/claude-bridge-healthcheck

EXPOSE 9223
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "9223"]
