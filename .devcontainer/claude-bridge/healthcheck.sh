#!/usr/bin/env sh
# TCP port probe on :9223. UCP exposes only POST /v1/messages — no /health
# endpoint — so we can't do HTTP-level. A bound port = uvicorn alive.
exec python3 -c "import socket,sys; s=socket.socket(); s.settimeout(2); s.connect(('127.0.0.1',9223)); s.close()" || exit 1
