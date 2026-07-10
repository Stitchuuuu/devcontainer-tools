// tokens skill — hardcoded pricing fallback (USD per 1M tokens).
// Mirror of PRICING_FALLBACK in lib/capture.py. Session 3 replaces the source
// with ~/.claude/tokens/pricing.json when refresh-pricing.sh ships.

const PRICING = {
  'claude-opus-4-7':   { in: 5.00,  cache_read: 0.50, cache_create: 10.00, out: 25.00 },
  'claude-opus-4-6':   { in: 5.00,  cache_read: 0.50, cache_create: 10.00, out: 25.00 },
  'claude-opus-4-5':   { in: 5.00,  cache_read: 0.50, cache_create: 10.00, out: 25.00 },
  'claude-opus-4-1':   { in: 15.00, cache_read: 1.50, cache_create: 30.00, out: 75.00 },
  'claude-sonnet-4-6': { in: 3.00,  cache_read: 0.30, cache_create: 6.00,  out: 15.00 },
  'claude-sonnet-4-5': { in: 3.00,  cache_read: 0.30, cache_create: 6.00,  out: 15.00 },
  'claude-sonnet-4':   { in: 3.00,  cache_read: 0.30, cache_create: 6.00,  out: 15.00 },
  'claude-haiku-4-5':  { in: 1.00,  cache_read: 0.10, cache_create: 2.00,  out: 5.00 },
  'claude-haiku-3-5':  { in: 0.80,  cache_read: 0.08, cache_create: 1.60,  out: 4.00 },
  'claude-haiku-3':    { in: 0.25,  cache_read: 0.03, cache_create: 0.50,  out: 1.25 },
};
const FALLBACK_MODEL = 'claude-opus-4-7';

function priceFor(model) {
  for (const key of Object.keys(PRICING)) {
    if (model && model.startsWith(key)) return PRICING[key];
  }
  return PRICING[FALLBACK_MODEL];
}

function costUsd(tokens, prices) {
  return (
    tokens.in * prices.in
    + tokens.cache_read * prices.cache_read
    + tokens.cache_create * prices.cache_create
    + tokens.out * prices.out
  ) / 1_000_000;
}

module.exports = { PRICING, FALLBACK_MODEL, priceFor, costUsd };
