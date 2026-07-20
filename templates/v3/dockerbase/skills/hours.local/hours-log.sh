#!/bin/sh
# Claude Code time-tracking hook for /user:hours
# Usage: sh hours-log.sh <event_name>
# Receives JSON on stdin from Claude Code hooks

EVENT="$1"
LOG_DIR="/workspace/.devcontainer/skills/hours.local/logs"
mkdir -p "$LOG_DIR"

INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | grep -o '"session_id":"[^"]*"' | head -1 | cut -d'"' -f4)
NOW=$(date -u +%s)
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
LOG_FILE="$LOG_DIR/$SESSION_ID.jsonl"

# tool_name for PostToolUse
TOOL_NAME=""
if [ "$EVENT" = "tool" ]; then
  TOOL_NAME=$(echo "$INPUT" | grep -o '"tool_name":"[^"]*"' | head -1 | cut -d'"' -f4)
fi

# Cost tracking on stop events (read transcript for token usage)
COST_PART=""
if [ "$EVENT" = "stop" ]; then
  TRANSCRIPT=$(echo "$INPUT" | grep -o '"transcript_path":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ] && command -v python3 >/dev/null 2>&1; then
    COST_PART=$(python3 -c "
import json, os

# Read previous cumulative totals from log
prev = {'in':0,'cache_read':0,'cache_create':0,'out':0}
log_path = '$LOG_FILE'
if os.path.exists(log_path):
    with open(log_path) as f:
        for line in reversed(f.readlines()):
            try:
                d = json.loads(line)
                if 'tokens_total' in d:
                    prev = d['tokens_total']
                    break
            except: pass

# Sum all tokens from transcript (cumulative)
total = {'in':0,'cache_read':0,'cache_create':0,'out':0}
with open('$TRANSCRIPT') as f:
    for line in f:
        try:
            d = json.loads(line)
            u = d.get('message',{}).get('usage',{})
            if u.get('input_tokens') is not None:
                total['in'] += u.get('input_tokens',0)
                total['cache_read'] += u.get('cache_read_input_tokens',0)
                total['cache_create'] += u.get('cache_creation_input_tokens',0)
                total['out'] += u.get('output_tokens',0)
        except: pass

# Incremental = total - previous
delta = {k: total[k] - prev.get(k,0) for k in total}

# Fallback pricing per MTok by model
PRICING_FALLBACK = {
    'claude-opus-4-6':   {'in':5.0,   'cache_read':0.50, 'cache_create':10.0,  'out':25.0},
    'claude-sonnet-4-6': {'in':3.0,   'cache_read':0.30, 'cache_create':6.0,   'out':15.0},
    'claude-haiku-4-5':  {'in':1.0,   'cache_read':0.10, 'cache_create':2.0,   'out':5.0},
}

# Try to load pricing from calibration file, create if missing
cal_path = '/workspace/.devcontainer/skills/hours.local/hours-calibration.json'
PRICING = dict(PRICING_FALLBACK)
if not os.path.exists(cal_path):
    cal_init = {
        'last_updated': '',
        'sources': [],
        'tjm_marche': {},
        'grille_heures': {},
        'api_pricing': {k: dict(v) for k, v in PRICING_FALLBACK.items()},
        'notes': 'Auto-generated. Run /user:hours-calibrate to populate.'
    }
    with open(cal_path, 'w') as f:
        json.dump(cal_init, f, indent=2)
try:
    with open(cal_path) as f:
        cal = json.load(f)
    ap = cal.get('api_pricing', {})
    if ap:
        PRICING = {}
        for model_key, model_prices in ap.items():
            PRICING[model_key] = model_prices
except: pass

# Detect model from transcript (last message with a model field)
model = 'unknown'
with open('$TRANSCRIPT') as f:
    for line in f:
        try:
            d = json.loads(line)
            m = d.get('message',{}).get('model','')
            if m:
                model = m
        except: pass

# Match model to pricing (prefix match for versioned names)
prices = None
for key in PRICING:
    if model.startswith(key):
        prices = PRICING[key]
        break
if not prices:
    prices = PRICING_FALLBACK.get('claude-opus-4-6')  # fallback

def calc_usd(t, p):
    return (t['in']*p['in'] + t['cache_read']*p['cache_read'] + t['cache_create']*p['cache_create'] + t['out']*p['out']) / 1_000_000

usd_total = calc_usd(total, prices)
usd_delta = calc_usd(delta, prices)

print(f',\"model\":\"{model}\",\"tokens\":{{\"in\":{delta[\"in\"]},\"cache_read\":{delta[\"cache_read\"]},\"cache_create\":{delta[\"cache_create\"]},\"out\":{delta[\"out\"]}}},\"cost_usd\":{usd_delta:.4f},\"tokens_total\":{{\"in\":{total[\"in\"]},\"cache_read\":{total[\"cache_read\"]},\"cache_create\":{total[\"cache_create\"]},\"out\":{total[\"out\"]}}},\"cost_usd_total\":{usd_total:.4f}')
" 2>/dev/null)
  fi
fi

# Log the event
if [ -n "$TOOL_NAME" ]; then
  echo "{\"ts\":\"$TIMESTAMP\",\"event\":\"$EVENT\",\"session\":\"$SESSION_ID\",\"tool\":\"$TOOL_NAME\"}" >> "$LOG_FILE"
else
  echo "{\"ts\":\"$TIMESTAMP\",\"event\":\"$EVENT\",\"session\":\"$SESSION_ID\"$COST_PART}" >> "$LOG_FILE"
fi

# Gap detection (>= 10 min since last stop)
if [ "$EVENT" = "prompt" ] && [ -f "$LOG_FILE" ]; then
  LAST_STOP=$(grep "\"event\":\"stop\"" "$LOG_FILE" | tail -1)
  if [ -n "$LAST_STOP" ]; then
    LAST_TS=$(echo "$LAST_STOP" | grep -o '"ts":"[^"]*"' | cut -d'"' -f4)
    LAST_EPOCH=$(date -u -d "${LAST_TS%Z}" +%s 2>/dev/null || echo "0")
    if [ "$LAST_EPOCH" != "0" ]; then
      GAP=$(( NOW - LAST_EPOCH ))
      GAP_MIN=$(( GAP / 60 ))
      if [ "$GAP_MIN" -ge 10 ]; then
        echo "{\"ts\":\"$TIMESTAMP\",\"event\":\"gap_detected\",\"session\":\"$SESSION_ID\",\"gap_min\":$GAP_MIN}" >> "$LOG_FILE"
        echo "⏱️ Gap de ${GAP_MIN} min detecte depuis la derniere reponse. C'etait de la reflexion/review ou une pause ?"
      fi
    fi
  fi
fi
