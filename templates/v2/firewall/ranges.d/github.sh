#!/bin/bash
# Fetch GitHub IP ranges (web, api, git) and output as aggregated CIDRs
curl -fsSL --connect-timeout 5 --max-time 15 https://api.github.com/meta \
  | jq -r '(.web + .api + .git)[]' \
  | aggregate -q
