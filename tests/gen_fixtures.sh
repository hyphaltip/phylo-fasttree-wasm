#!/usr/bin/env bash
# Regenerate native reference trees for tests/node_test.js.
# Requires a strict-FP native FastTree build (see README.md).
set -euo pipefail

FT=${FT:-/tmp/opencode/fasttree-native-strict}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

# name|fasta|args passed to native FastTree
CASES=(
  'nt_small|nt_small.fa|-nt'
  'nt_small_gtr|nt_small.fa|-nt -gtr'
  'nt_small_gamma|nt_small.fa|-nt -gamma'
  'nt_small_gtr_gamma|nt_small.fa|-nt -gtr -gamma'
  'nt_small_nosupport|nt_small.fa|-nt -nosupport'
  'nt_small_fastest|nt_small.fa|-nt -fastest'
  'nt_small_slow|nt_small.fa|-nt -slow'
  'nt_small_cat4|nt_small.fa|-nt -cat 4'
  'nt_small_nj|nt_small.fa|-nt -nj'
  'nt_small_bionj|nt_small.fa|-nt -bionj'
  'nt_small_noml|nt_small.fa|-nt -noml'
  'nt_small_logdist|nt_small.fa|-nt -logdist'
  'nt_small_rawdist|nt_small.fa|-nt -rawdist'
  'nt_small_seed|nt_small.fa|-nt -seed 42'
  'nt_small_mlnni2|nt_small.fa|-nt -mlnni 2'
  'nt_small_spr2|nt_small.fa|-nt -spr 2'
  'nt_small_topm3|nt_small.fa|-nt -topm 3'
  'nt_large|nt_large.fa|-nt'
  'nt_large_gtr_gamma|nt_large.fa|-nt -gtr -gamma'
  'nt_large_fastest|nt_large.fa|-nt -fastest'
  'aa_small|aa_small.fa|'
  'aa_small_lg|aa_small.fa|-lg'
  'aa_small_wag|aa_small.fa|-wag'
  'aa_small_gamma|aa_small.fa|-gamma'
  'aa_small_nosupport|aa_small.fa|-nosupport'
  'aa_small_cat4|aa_small.fa|-cat 4'
  'aa_small_noml|aa_small.fa|-noml'
  'aa_small_slow|aa_small.fa|-slow'
  'aa_small_fastest|aa_small.fa|-fastest'
  'aa_small_mlnni2|aa_small.fa|-mlnni 2'
  'aa_small_seed|aa_small.fa|-seed 42'
  'aa_large|aa_large.fa|'
  'aa_large_lg_gamma|aa_large.fa|-lg -gamma'
  'aa_large_fastest|aa_large.fa|-fastest'
)

for entry in "${CASES[@]}"; do
  IFS='|' read -r name fasta args <<< "$entry"
  # shellcheck disable=SC2086
  "$FT" -quiet -nopr $args "fixtures/$fasta" > "fixtures/$name.newick"
  echo "$name: $(wc -c < "fixtures/$name.newick") bytes"
done
