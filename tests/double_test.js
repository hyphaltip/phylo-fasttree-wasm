// Double-precision build (`make test-double`) vs a strict double-precision
// native FastTree (FT_DOUBLE=path, built with
//   gcc -O3 -DUSE_DOUBLE -DNO_SSE -DNDEBUG -ffp-contract=off FastTree.c -lm).
// Newick text must match exactly, except that numbers may differ by ≤1e-6:
// WASM uses Rust's libm, native uses the system libm, and in double precision
// their last-ulp differences can reach the printed digits under -gamma.
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const { buildTree, fastTreeVersion } = require('./pkg_double/phylo_fasttree_wasm.js');

const FT = process.env.FT_DOUBLE || '/tmp/fasttree-native-double-strict';
if (!fs.existsSync(FT)) { console.error(`native double FastTree not found at ${FT} (set FT_DOUBLE)`); process.exit(2); }
const fx = n => path.join(__dirname, '..', 'fixtures', n);
const NUM = /[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g;

function sameWithin(a, b, tol) {
  const ta = a.split(NUM), tb = b.split(NUM);
  const na = a.match(NUM) || [], nb = b.match(NUM) || [];
  if (ta.join('\u0000') !== tb.join('\u0000') || na.length !== nb.length) return { ok: false, max: Infinity };
  let max = 0;
  for (let i = 0; i < na.length; i++) max = Math.max(max, Math.abs(parseFloat(na[i]) - parseFloat(nb[i])));
  return { ok: max <= tol, max };
}

const cases = [
  ['nt_small.fa', ['-nt']], ['nt_small.fa', ['-nt', '-gtr']], ['nt_small.fa', ['-nt', '-gamma']],
  ['nt_medium.fa', ['-nt', '-gtr', '-gamma']], ['nt_large.fa', ['-nt']],
  ['aa_small.fa', []], ['aa_small.fa', ['-lg']], ['aa_small.fa', ['-wag', '-gamma']],
  ['aa_large.fa', ['-lg', '-gamma']],
];
console.log(`FastTree version: ${fastTreeVersion()} (double precision build)`);
let fail = 0, exact = 0;
for (const [f, args] of cases) {
  const native = execFileSync(FT, [...args, fx(f)], { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
  const wasm = buildTree(fs.readFileSync(fx(f), 'utf8'), args).trim();
  const label = `${f} ${args.join(' ') || '(defaults)'}`;
  if (native === wasm) { exact++; continue; }
  const r = sameWithin(native, wasm, 1e-6);
  if (r.ok) console.log(`  ~ ${label}: numbers within ${r.max.toExponential(1)}`);
  else { fail++; console.error(`FAIL ${label} (max numeric diff ${r.max})`); }
}
console.log(`${cases.length - fail} passed (${exact} byte-identical), ${fail} failed`);
process.exit(fail ? 1 : 0);
