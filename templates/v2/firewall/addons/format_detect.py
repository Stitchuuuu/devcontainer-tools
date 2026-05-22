"""
format_detect.py — mitmproxy addon for Phase A2.

Inspects the body of POST/PUT/PATCH requests and rejects archive payloads,
both as raw magic bytes and as base64-wrapped archives. Goal: prevent
data exfiltration by uploading a zipped/gzipped/7z'd blob to an otherwise
allow-listed endpoint.

Both detectors are toggled by defaults flags in policy.compiled.yaml
(block_archive_magic / block_archive_in_base64). Base64 scan is capped at the
first 64 KB of body to bound CPU on large uploads.
"""

import base64
import json
import re
import sys
import time
import traceback

from mitmproxy import http
# See policy_enforce.py + knowledge/firewall.md § "mitmproxy bundle: ruamel.yaml only".
from ruamel.yaml import YAML

_YAML = YAML(typ='safe')
POLICY_PATH = "/var/run/devcontainer-firewall/policy.compiled.yaml"
BLOCKS_LOG = "/var/log/mitmproxy-blocks.log"
ADDON_NAME = "format_detect"

try:
    with open(POLICY_PATH) as _f:
        _POLICY = _YAML.load(_f) or {}
except Exception as _exc:
    print(f"[format_detect] FATAL loading {POLICY_PATH}: {_exc}", file=sys.stderr)
    _POLICY = {}

_DEFAULTS = _POLICY.get("defaults", {}) or {}
BLOCK_MAGIC = bool(_DEFAULTS.get("block_archive_magic", True))
BLOCK_B64 = bool(_DEFAULTS.get("block_archive_in_base64", True))
# Fallback if policy_enforce.py didn't run first (or wasn't loaded). Normally
# policy_enforce sets flow._enforcement_mode per-request based on host/endpoint
# override resolution — we honour that when present.
_DEFAULT_MODE = _DEFAULTS.get("enforcement_mode", "block")

ARCHIVE_MAGIC = {
    b"PK\x03\x04": "zip",
    b"\x1f\x8b": "gzip",
    b"BZh": "bzip2",
    b"7z\xbc\xaf\x27\x1c": "7z",
    b"Rar!\x1a\x07": "rar",
    b"\xfd7zXZ\x00": "xz",
}
BASE64_LONG_RE = re.compile(rb"[A-Za-z0-9+/=]{200,}")
WRITE_METHODS = {"POST", "PUT", "PATCH"}
B64_SCAN_LIMIT = 65536


def _has_archive_magic(content: bytes):
    head = content[:1024]
    for magic, fmt in ARCHIVE_MAGIC.items():
        if magic in head:
            return fmt
    return None


def _has_archive_in_b64(content: bytes):
    for m in BASE64_LONG_RE.finditer(content[:B64_SCAN_LIMIT]):
        try:
            decoded = base64.b64decode(m.group(), validate=False)
        except Exception:
            continue
        fmt = _has_archive_magic(decoded)
        if fmt:
            return fmt
    return None


def _record_block(flow: http.HTTPFlow, reason: str, mode: str) -> None:
    """Append a JSON line to /var/log/mitmproxy-blocks.log so `firewall-blocks`
    can summarise recent blocks. Best-effort: never re-raise.

    `mode` is `block` (request rejected) or `warn` (request passed but rule
    would have fired). Used by firewall-blocks to distinguish observation."""
    try:
        entry = {
            "ts": time.time(),
            "addon": ADDON_NAME,
            "mode": mode,
            "method": flow.request.method,
            "host": flow.request.host,
            "path": flow.request.path,
            "code": 403,
            "reason": reason,
            "ua": flow.request.headers.get("user-agent", "")[:120],
        }
        with open(BLOCKS_LOG, "a") as f:
            f.write(json.dumps(entry, separators=(",", ":")) + "\n")
    except Exception:
        pass


def _block(flow: http.HTTPFlow, reason: str) -> None:
    """Block (or warn) on an archive payload match. Mode is read from
    `flow._enforcement_mode` if policy_enforce.py ran first and set it ;
    otherwise falls back to format_detect's own DEFAULT_MODE."""
    mode = getattr(flow, "_enforcement_mode", _DEFAULT_MODE)
    _record_block(flow, reason, mode)
    if mode == "warn":
        return
    flow.response = http.Response.make(
        403,
        reason.encode(),
        {"X-Block-Reason": reason, "Content-Type": "text/plain"},
    )


def request(flow: http.HTTPFlow) -> None:
    try:
        if flow.request.method not in WRITE_METHODS:
            return
        content = flow.request.content or b""
        if BLOCK_MAGIC:
            fmt = _has_archive_magic(content)
            if fmt:
                return _block(flow, f"archive_magic:{fmt}")
        if BLOCK_B64:
            fmt = _has_archive_in_b64(content)
            if fmt:
                return _block(flow, f"archive_in_base64:{fmt}")
    except Exception as exc:
        print(
            f"[format_detect] EXCEPTION: {exc}\n{traceback.format_exc()}",
            file=sys.stderr,
        )
        _block(flow, f"addon_error:format_detect:{type(exc).__name__}")
