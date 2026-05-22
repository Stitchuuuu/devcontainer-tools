#!/bin/bash
# Fetch BunnyCDN edge server CIDRs (used by npm registry)
curl -fsSL --connect-timeout 5 --max-time 15 \
  https://bunnycdn.com/api/system/edgeserverlist/plain
