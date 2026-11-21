const fs = require('fs');
const path = require('path');
const {
  buildTree,
  buildTreeNT,
  buildTreeProtein,
  fastTreeVersion,
} = require('./pkg/phylo_fasttree_wasm.js');

const root = path.join(__dirname, '..');
let failures = 0;
let passed = 0;

function check(name, got, expected) {
  const g = got.trim();
  const e = expected.trim();
  if (g === e) {
    passed++;
  } else {
    failures++;
    console.error(`FAIL ${name}`);
    console.error(`  expected: ${e}`);
    console.error(`  got:      ${g}`);
  }
}

function fasta(name) {
  return fs.readFileSync(path.join(root, 'fixtures', name), 'utf8');
}

function expected(name) {
  return fs.readFileSync(path.join(root, 'fixtures', name), 'utf8');
}

console.log(`FastTree version: ${fastTreeVersion()}`);

// Convenience wrappers and repeated/interleaved calls.
check('nt_small (convenience)', buildTreeNT(fasta('nt_small.fa')), expected('nt_small.newick'));
check('nt_small (raw args)', buildTree(fasta('nt_small.fa'), ['-nt']), expected('nt_small.newick'));
check('nt_medium', buildTreeNT(fasta('nt_medium.fa')), expected('nt_medium.newick'));
check('aa_small', buildTreeProtein(fasta('aa_small.fa')), expected('aa_small.newick'));

check('repeat call #1', buildTreeNT(fasta('nt_small.fa')), expected('nt_small.newick'));
check('repeat call #2', buildTreeNT(fasta('nt_small.fa')), expected('nt_small.newick'));
check('interleaved', buildTreeNT(fasta('nt_medium.fa')), expected('nt_medium.newick'));
check('back to small', buildTreeNT(fasta('nt_small.fa')), expected('nt_small.newick'));

// Switching alphabets must rebuild the residue lookup cache.
check('protein after nt', buildTreeProtein(fasta('aa_small.fa')), expected('aa_small.newick'));
check('nt after protein', buildTreeNT(fasta('nt_small.fa')), expected('nt_small.newick'));
check('protein after nt again', buildTreeProtein(fasta('aa_small.fa')), expected('aa_small.newick'));

// Full option matrix: [name, fasta, args, expected].
const cases = [
  ['nt -gtr', 'nt_small.fa', ['-nt', '-gtr'], 'nt_small_gtr.newick'],
  ['nt -gamma', 'nt_small.fa', ['-nt', '-gamma'], 'nt_small_gamma.newick'],
  ['nt -gtr -gamma', 'nt_small.fa', ['-nt', '-gtr', '-gamma'], 'nt_small_gtr_gamma.newick'],
  ['nt -nosupport', 'nt_small.fa', ['-nt', '-nosupport'], 'nt_small_nosupport.newick'],
  ['nt -fastest', 'nt_small.fa', ['-nt', '-fastest'], 'nt_small_fastest.newick'],
  ['nt -slow', 'nt_small.fa', ['-nt', '-slow'], 'nt_small_slow.newick'],
  ['nt -cat 4', 'nt_small.fa', ['-nt', '-cat', '4'], 'nt_small_cat4.newick'],
  ['nt -nj', 'nt_small.fa', ['-nt', '-nj'], 'nt_small_nj.newick'],
  ['nt -bionj', 'nt_small.fa', ['-nt', '-bionj'], 'nt_small_bionj.newick'],
  ['nt -noml', 'nt_small.fa', ['-nt', '-noml'], 'nt_small_noml.newick'],
  ['nt -logdist', 'nt_small.fa', ['-nt', '-logdist'], 'nt_small_logdist.newick'],
  ['nt -rawdist', 'nt_small.fa', ['-nt', '-rawdist'], 'nt_small_rawdist.newick'],
  ['nt -seed 42', 'nt_small.fa', ['-nt', '-seed', '42'], 'nt_small_seed.newick'],
  ['nt -mlnni 2', 'nt_small.fa', ['-nt', '-mlnni', '2'], 'nt_small_mlnni2.newick'],
  ['nt -spr 2', 'nt_small.fa', ['-nt', '-spr', '2'], 'nt_small_spr2.newick'],
  ['nt -topm 3', 'nt_small.fa', ['-nt', '-topm', '3'], 'nt_small_topm3.newick'],
  ['nt large', 'nt_large.fa', ['-nt'], 'nt_large.newick'],
  ['nt large -gtr -gamma', 'nt_large.fa', ['-nt', '-gtr', '-gamma'], 'nt_large_gtr_gamma.newick'],
  ['nt large -fastest', 'nt_large.fa', ['-nt', '-fastest'], 'nt_large_fastest.newick'],
  ['aa -lg', 'aa_small.fa', ['-lg'], 'aa_small_lg.newick'],
  ['aa -wag', 'aa_small.fa', ['-wag'], 'aa_small_wag.newick'],
  ['aa -gamma', 'aa_small.fa', ['-gamma'], 'aa_small_gamma.newick'],
  ['aa -nosupport', 'aa_small.fa', ['-nosupport'], 'aa_small_nosupport.newick'],
  ['aa -cat 4', 'aa_small.fa', ['-cat', '4'], 'aa_small_cat4.newick'],
  ['aa -noml', 'aa_small.fa', ['-noml'], 'aa_small_noml.newick'],
  ['aa -slow', 'aa_small.fa', ['-slow'], 'aa_small_slow.newick'],
  ['aa -fastest', 'aa_small.fa', ['-fastest'], 'aa_small_fastest.newick'],
  ['aa -mlnni 2', 'aa_small.fa', ['-mlnni', '2'], 'aa_small_mlnni2.newick'],
  ['aa -seed 42', 'aa_small.fa', ['-seed', '42'], 'aa_small_seed.newick'],
  ['aa large', 'aa_large.fa', [], 'aa_large.newick'],
  ['aa large -lg -gamma', 'aa_large.fa', ['-lg', '-gamma'], 'aa_large_lg_gamma.newick'],
  ['aa large -fastest', 'aa_large.fa', ['-fastest'], 'aa_large_fastest.newick'],
];

for (const [name, fa, args, exp] of cases) {
  check(name, buildTree(fasta(fa), args), expected(exp));
}

// Stress reset with a long mixed sequence that changes alphabet and options.
const stress = [
  ['nt -gtr', 'nt_small.fa', ['-nt', '-gtr'], 'nt_small_gtr.newick'],
  ['aa -lg', 'aa_small.fa', ['-lg'], 'aa_small_lg.newick'],
  ['nt -gamma', 'nt_small.fa', ['-nt', '-gamma'], 'nt_small_gamma.newick'],
  ['aa -wag', 'aa_small.fa', ['-wag'], 'aa_small_wag.newick'],
  ['nt large -fastest', 'nt_large.fa', ['-nt', '-fastest'], 'nt_large_fastest.newick'],
  ['aa large', 'aa_large.fa', [], 'aa_large.newick'],
  ['nt small', 'nt_small.fa', ['-nt'], 'nt_small.newick'],
  ['aa small', 'aa_small.fa', [], 'aa_small.newick'],
];
for (const [name, fa, args, exp] of stress) {
  check(`stress ${name}`, buildTree(fasta(fa), args), expected(exp));
}

// Invalid input should throw rather than trap the module.
try {
  buildTreeNT('>only_one\nACGT\n');
  failures++;
  console.error('FAIL invalid input should throw');
} catch (e) {
  passed++;
}

try {
  buildTreeNT('>a\nACGT\n>b\nACGZ\n');
  failures++;
  console.error('FAIL invalid residue should throw');
} catch (e) {
  passed++;
}

// FastTree's own exit(1) path must produce a catchable error, not an
// `unreachable` trap. `-close` without `-top` is rejected by the C code after
// the Rust pre-flight has accepted the arguments.
try {
  buildTree('>a\nACGTACGT\n>b\nACGTACGA\n', ['-nt', '-notop', '-close', '0.5']);
  failures++;
  console.error('FAIL FastTree exit should throw a catchable error');
} catch (e) {
  const message = String(e && (e.message || e));
  if (/FastTree exited with status/.test(message) && /-close/.test(message)) {
    passed++;
  } else {
    failures++;
    console.error(`FAIL FastTree exit error message: ${message}`);
  }
}

console.log(`${passed} passed, ${failures} failed`);
if (failures) {
  process.exit(1);
}
console.log('All tests passed');
