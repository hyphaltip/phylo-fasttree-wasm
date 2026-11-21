# phylo-fasttree-wasm

Upstream [FastTree 2.1.11](http://www.microbesonline.org/fasttree/) compiled to
`wasm32-unknown-unknown` and wrapped in a small `wasm-bindgen` API. It runs in a
browser (or Node) with **no filesystem, no stdin, no emscripten runtime, and no
external JS glue beyond wasm-bindgen's standard output**.

## Why not emscripten?

Emscripten is the easy way to compile a C command-line program to WASM, but it
brings its own JS runtime, virtual filesystem, and linear memory. Passing a
FASTA string from another Rust/wasm module into an emscripten module means
copying data across two heaps via JS. This crate links the C code directly into
the same WASM module as the Rust pipeline code, so the API is simply:

```
FASTA string  ->  Newick string
```

## Usage from Rust

```rust
use phylo_fasttree_wasm::infer;

let fasta = ">a\nACGT\n>b\nACGA\n";
let newick = infer(fasta, &["-nt"])?;
```

`infer()` is a plain Rust function, so it composes with other Rust WASM tools
in a pipeline. `args` are FastTree command-line options (`-nt`, `-gtr`, `-gamma`,
`-nosupport`, ...). Options that take a value (e.g. `-cat 4`, `-seed 42`,
`-mlnni 2`) accept the value as the following array element. File-based options
(`-out`, `-log`, `-intree`, `-constraints`, `-matrix`, `-trans`, `-makematrix`)
are rejected.

### JS / wasm-bindgen API

| Export | Signature |
| --- | --- |
| `buildTree` | `(fasta: string, args: string[]) -> string` |
| `buildTreeNT` | `(fasta: string) -> string` |
| `buildTreeProtein` | `(fasta: string) -> string` |
| `fastTreeVersion` | `() -> string` |
| `fastTreePrecision` | `() -> "single" \| "double"` |

All functions throw a normal JavaScript `Error` on invalid input, including
FastTree's own `exit(1)` paths (e.g. incompatible option combinations); the
error includes FastTree's own stderr message in `e.message`.

**Restore the stack pointer after a caught error.** An `exit()` inside FastTree
throws straight out of WASM, skipping the C epilogues, so the shadow-stack
pointer drifts downward with every failure and corrupts memory after a few
hundred errors. Wrap calls like this (with `--target web`, `init()` resolves to
the module exports):

```js
const wasm = await init();
function safeBuildTree(fasta, args) {
  const sp = wasm.__wbindgen_add_to_stack_pointer(0);
  try { return buildTree(fasta, args); }
  catch (e) { wasm.__wbindgen_add_to_stack_pointer(sp - wasm.__wbindgen_add_to_stack_pointer(0)); throw e; }
}
```

The returned Newick string has the trailing newline stripped.

## Build

Requires: Rust with the `wasm32-unknown-unknown` target, clang 14+ with the
WebAssembly target, `llvm-ar`, and `wasm-bindgen-cli` 0.2.83.

```sh
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/phylo_fasttree_wasm.wasm \
  --target nodejs --out-dir tests/pkg
node tests/node_test.js
```

For the browser, use `--target web` (or `--target bundler`) instead of
`--target nodejs` when running `wasm-bindgen`. `make browser-test` runs a
headless Chromium smoke test; `make browser-demo` regenerates the glue for
`tests/browser.html` (serve `tests/` over HTTP and open that page).

## Precision

FastTree compiles in single precision by default, or in double precision with
`-DUSE_DOUBLE`. Bioconda's `fasttree` package is the double-precision build.
Double precision mainly matters for very similar sequences: it allows much
shorter minimum branch lengths, so nucleotide trees with near-zero branches can
differ in both branch lengths and topology. Protein trees typically differ only
in the 4th decimal place.

This crate builds either one:

```sh
make web            # single precision → web/pkg_web/phylo_fasttree_wasm.*
make web-double     # double precision → web/pkg_web_double/phylo_fasttree_wasm_double.*
# or: cargo build --target wasm32-unknown-unknown --release --features double
```

`fastTreePrecision()` reports which build is loaded.

**Speed.** In WebAssembly the two run at the same speed. The WASM build has no
SSE (`-DNO_SSE`), and scalar f32 and f64 arithmetic cost the same, so single
precision's native speed advantage, which comes from its SSE code, doesn't
apply. Simulated alignments on Node 22 (aarch64), best of two runs:

| Alignment | Options | single | double |
| --- | --- | --- | --- |
| 200 proteins × 400 aa | `-lg -gamma` | 17.9 s | 18.4 s (1.03×) |
| 300 sequences × 1000 nt | `-nt -gtr -gamma` | 14.2 s | 13.8 s (0.98×) |

Double precision uses more memory for likelihood vectors, which only matters
near the browser's memory ceiling. The `.wasm` grows by 7 KB (307 KB vs
301 KB).

**Correctness.** `make test-double FT_DOUBLE=/path/to/native-double` compares
the double build against a strict double-precision native FastTree:

```sh
gcc -O3 -DUSE_DOUBLE -DNO_SSE -DNDEBUG -ffp-contract=off -o fasttree-native-double FastTree.c -lm
```

Seven of the nine cases are byte-identical. The two `-gamma` cases match
except for numbers ≤ 1e-7 apart: WASM uses Rust's `libm` and native uses the
system libm, and in double precision their last-bit differences can reach the
printed digits. The test allows numeric differences up to 1e-6, with all
other Newick text identical.

## How it works

- `vendor/FastTree.c` is upstream FastTree 2.1.11 with three surgical edits:
  `main()` is renamed to `fasttree_main_impl()`, the final `exit(0)` returns the
  captured tree instead, and the residue lookup cache in `SeqToProfile()` is
  rebuilt on every call so switching between protein and nucleotide alignments
  in one WASM instance is safe. The inference algorithm is untouched.
  `vendor/src/ft_reset.c` resets the remaining global options and PRNG state.
- `vendor/src/ft_compat.c` implements in-memory `FILE*`, a small `printf`,
  `qsort`, ctype helpers, numeric conversions, and OS stubs (`isatty`,
  `gettimeofday`). `stdin` is the FASTA buffer; `stdout` is the Newick buffer.
- `src/libc_shim.rs` implements the remaining libc symbols in Rust: an
  allocator (malloc/free/realloc forwarded to Rust's dlmalloc, with live
  blocks tracked so anything left by an aborted run is swept on the next call),
  string/memory functions, `log`/`exp`/`sqrt`/`fabs` via the `libm` crate, and
  float formatting for `printf`.
- `src/lib.rs` validates input, assembles `argv`, calls FastTree, and exposes
  the wasm-bindgen API.

## Correctness

`tests/node_test.js` runs 53 byte-for-byte comparisons against native FastTree
2.1.11 reference trees in `fixtures/*.newick`. Coverage includes:

- small and large nucleotide/protein alignments
- option matrices: `-gtr`, `-gamma`, `-nosupport`, `-fastest`, `-slow`,
  `-cat`, `-nj`, `-bionj`, `-noml`, `-logdist`, `-rawdist`, `-seed`,
  `-mlnni`, `-spr`, `-topm`, `-lg`, `-wag`
- repeated calls, interleaved calls, and mixed protein↔nucleotide sequences
- invalid input handling

Regenerate the reference trees with `tests/gen_fixtures.sh` (set `FT` to the
path of the native binary; default is `/tmp/opencode/fasttree-native-strict`).

The reference trees in `fixtures/*.newick` must be generated with strict
floating-point native builds:

```sh
gcc -O3 -DNO_SSE -DNDEBUG -ffp-contract=off \
  -o fasttree-native FastTree.c -lm
```

`-ffp-contract=off` is essential: on some architectures (notably arm64) GCC
otherwise fuses multiply/add operations into FMAs, which changes protein branch
lengths and support values relative to the WASM build (clang does not fuse by
default for `wasm32-unknown-unknown`).

## Limitations

- Single alignment per call; multi-alignment (`-n` > 1) is not exposed.
- No local bootstrap support values unless `-nosupport` is passed (FastTree
  defaults to SH-like local support; pass `-nosupport` for faster runs).
- Unknown options are passed straight through, so check FastTree's own docs
  for behavior.
- On unexpected internal failure FastTree calls `exit()`; the shim turns that
  into a catchable JS error containing FastTree's stderr message instead of a
  `RuntimeError: unreachable` trap. Input validation in Rust still catches the
  common parse failures before entering WASM.

## License

The vendored FastTree C code is GPL-2.0-or-later (Copyright 2008-2015 The
Regents of the University of California). This crate is therefore licensed
GPL-2.0-or-later.
