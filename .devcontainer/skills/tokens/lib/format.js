// tokens skill — SI-compact number formatter.
// Rule: 3 significant digits, trim trailing zeros, ≤ 4 chars.
// USD variant appends '$' (≤ 5 chars). Reclassify after rounding to
// cross tier boundaries cleanly (999999 → 1M, not 1000K).

const SUFFIXES = ['', 'K', 'M', 'B', 'T'];

function decimalsFor(scaled) {
  if (scaled >= 100) return 0;
  if (scaled >= 10) return 1;
  return 2;
}

function format3sig(abs, tier) {
  const scaled = abs / Math.pow(1000, tier);
  const dec = decimalsFor(scaled);
  const rounded = Number(scaled.toFixed(dec));
  return { scaled, dec, rounded };
}

function compact(n, { sigFigsBelowK = false } = {}) {
  if (n === 0) return '0';
  const neg = n < 0;
  const abs = Math.abs(n);
  const sign = neg ? '-' : '';

  if (abs < 1000 && !sigFigsBelowK) {
    return sign + String(Math.round(abs));
  }

  let tier = abs < 1000 ? 0 : Math.min(Math.floor(Math.log10(abs) / 3), SUFFIXES.length - 1);
  let { dec, rounded } = format3sig(abs, tier);

  if (rounded >= 1000 && tier < SUFFIXES.length - 1) {
    tier += 1;
    ({ dec, rounded } = format3sig(abs, tier));
  }

  let s = rounded.toFixed(dec);
  if (s.includes('.')) s = s.replace(/0+$/, '').replace(/\.$/, '');
  return sign + s + SUFFIXES[tier];
}

function compactUSD(n) {
  return compact(n, { sigFigsBelowK: true }) + '$';
}

module.exports = { compact, compactUSD };

if (require.main === module) {
  const assert = require('node:assert/strict');
  const cases = [
    [0, '0'],
    [12, '12'],
    [999, '999'],
    [1000, '1K'],
    [1234, '1.23K'],
    [12345, '12.3K'],
    [123456, '123K'],
    [999999, '1M'],
    [1234567, '1.23M'],
    [12345678, '12.3M'],
    [999999999, '1B'],
    [1234567890, '1.23B'],
  ];
  for (const [input, expected] of cases) {
    assert.equal(compact(input), expected, `compact(${input})`);
  }
  const usdCases = [
    [0, '0$'],
    [1.23, '1.23$'],
    [12345, '12.3K$'],
    [999999, '1M$'],
  ];
  for (const [input, expected] of usdCases) {
    assert.equal(compactUSD(input), expected, `compactUSD(${input})`);
  }
  console.log(`lib/format.js: ${cases.length + usdCases.length}/${cases.length + usdCases.length} ok`);
}
