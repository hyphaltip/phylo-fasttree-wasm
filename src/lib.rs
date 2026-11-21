//! FastTree (upstream 2.1.11) as a self-contained `wasm32-unknown-unknown`
//! module with a small wasm-bindgen API.
//!
//! The C implementation is vendored in `vendor/`, compiled directly to
//! WebAssembly by `build.rs`, and linked with a minimal in-wasm libc shim.
//! No filesystem, stdin, emscripten runtime, or external JS is required.
//!
//! ```ignore
//! use phylo_fasttree_wasm::build_tree_js;
//! let fasta = ">a\nACGT\n>b\nACGA\n";
//! let newick = build_tree_js(fasta, vec!["-nt".into()]).unwrap();
//! ```

#[cfg(target_arch = "wasm32")]
mod libc_shim;

#[cfg(target_arch = "wasm32")]
use std::ffi::{CStr, CString};
#[cfg(target_arch = "wasm32")]
use std::os::raw::{c_char, c_void};
#[cfg(target_arch = "wasm32")]
use std::ptr;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn fasttree_build(
        fasta: *const u8,
        len: usize,
        argc: i32,
        argv: *const *const c_char,
    ) -> *mut c_char;
    fn free(p: *mut c_void);
    fn ft_clear_error();
    fn ft_last_error() -> i32;
    fn ft_take_stderr() -> *mut c_char;
    fn ft_reset_alloc();
    fn ft_reset_output();
}

const FORBIDDEN_ARGS: &[&str] = &[
    "-out", "-log", "-intree", "-intree1", "-constraints", "-matrix", "-trans",
    "-makematrix", "-help", "-expert",
];

/// FastTree options that consume the following argument as a value.
const VALUE_OPTIONS: &[&str] = &[
    "-verbose", "-boot", "-seed", "-topm", "-close", "-refresh", "-nni",
    "-spr", "-sprlength", "-mlnni", "-mlacc", "-cat", "-constraintWeight",
];

/// Validate the FASTA text that we are about to hand to C.
///
/// FastTree's own parser calls `exit(1)` from many places; catching the common
/// input errors here keeps the WASM instance alive and returns a useful error.
fn validate_fasta(fasta: &str, nucleotide: bool) -> Result<(), String> {
    if fasta.trim().is_empty() {
        return Err("empty FASTA input".to_string());
    }

    let mut n_seqs = 0usize;
    let mut expected_len: Option<usize> = None;
    let mut have_header = false;
    let mut current_len = 0usize;

    let allowed_nt = b"ACGTURYKMSWBDHVNX-.?";
    let allowed_aa = b"ARNDCQEGHILKMFPSTWYVBJZXU-.?*";

    for (lineno, raw_line) in fasta.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('>') {
            if name.trim().is_empty() {
                return Err(format!("line {}: empty sequence name", lineno + 1));
            }
            if have_header && current_len == 0 {
                return Err(format!("line {}: sequence '{}' has no residues", lineno, n_seqs));
            }
            if have_header {
                match expected_len {
                    None => expected_len = Some(current_len),
                    Some(n) if n != current_len => {
                        return Err(format!(
                            "line {}: sequence {} has length {}, expected {}",
                            lineno + 1,
                            n_seqs,
                            current_len,
                            n
                        ));
                    }
                    _ => {}
                }
            }
            have_header = true;
            current_len = 0;
            n_seqs += 1;
            continue;
        }

        if !have_header {
            return Err(format!("line {}: sequence data before a '>' header", lineno + 1));
        }

        let allowed: &[u8] = if nucleotide { allowed_nt } else { allowed_aa };
        for &b in line.as_bytes() {
            let up = b.to_ascii_uppercase();
            if !allowed.contains(&up) {
                return Err(format!(
                    "line {}: invalid residue '{}'",
                    lineno + 1,
                    b as char
                ));
            }
        }
        current_len += line.len();
    }

    if have_header {
        match expected_len {
            None => expected_len = Some(current_len),
            Some(n) if n != current_len => {
                return Err(format!(
                    "sequence {} has length {}, expected {}",
                    n_seqs, current_len, n
                ));
            }
            _ => {}
        }
    }

    if n_seqs < 2 {
        return Err("need at least two sequences to build a tree".to_string());
    }
    match expected_len {
        Some(0) => Err("alignment has zero columns".to_string()),
        None => Err("no sequences found".to_string()),
        Some(_) => Ok(()),
    }
}

/// Pure Rust API (usable from a pipeline of Rust wasm modules).
///
/// `args` are FastTree command-line options, e.g. `["-nt", "-gtr", "-gamma"]`.
/// Do not pass a positional alignment filename or any file-based option.
#[cfg(target_arch = "wasm32")]
pub fn infer(fasta: &str, args: &[&str]) -> Result<String, String> {
    let nucleotide = args.iter().any(|a| *a == "-nt");
    validate_fasta(fasta, nucleotide)?;

    let mut options: Vec<&str> = Vec::with_capacity(args.len() + 3);
    options.push("FastTree");
    // Keep logs out of stderr; the tree always goes to the captured stdout.
    if !args.iter().any(|a| *a == "-quiet") {
        options.push("-quiet");
    }
    if !args.iter().any(|a| *a == "-nopr") {
        options.push("-nopr");
    }
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if FORBIDDEN_ARGS.contains(&a) {
            return Err(format!("option '{}' needs a filesystem and is not supported", a));
        }
        if !a.starts_with('-') || a.starts_with("--") {
            return Err(format!("unexpected positional argument '{}'", a));
        }
        options.push(a);
        if VALUE_OPTIONS.contains(&a) {
            i += 1;
            if i >= args.len() {
                return Err(format!("option '{}' requires a value", a));
            }
            let value = args[i];
            if value.starts_with('-') {
                return Err(format!("option '{}' requires a value, got '{}'", a, value));
            }
            options.push(value);
        }
        i += 1;
    }

    let c_args: Vec<CString> = options
        .iter()
        .map(|s| CString::new(*s).map_err(|_| "argument contains a NUL byte".to_string()))
        .collect::<Result<_, _>>()?;
    let mut argv: Vec<*const c_char> = c_args.iter().map(|s| s.as_ptr()).collect();
    argv.push(ptr::null());

    unsafe {
        ft_clear_error();
        // Release C stream buffers first (nulls their pointers), then sweep
        // anything a previous (possibly aborted) run left allocated.
        ft_reset_output();
        ft_reset_alloc();
        let out = fasttree_build(fasta.as_ptr(), fasta.len(), argv.len() as i32 - 1, argv.as_ptr());
        if out.is_null() {
            let code = ft_last_error();
            let stderr = ft_take_stderr();
            let mut msg = if code != 0 {
                format!("FastTree exited with status {}", code)
            } else {
                "FastTree returned no tree".to_string()
            };
            if !stderr.is_null() {
                let detail = CStr::from_ptr(stderr).to_string_lossy();
                if !detail.trim().is_empty() {
                    msg.push_str(": ");
                    msg.push_str(detail.trim());
                }
                free(stderr as *mut c_void);
            }
            return Err(msg);
        }
        let newick = CStr::from_ptr(out).to_string_lossy().into_owned();
        free(out as *mut c_void);
        Ok(newick.trim_end_matches(|c| c == '\n' || c == '\r').to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn infer(_fasta: &str, _args: &[&str]) -> Result<String, String> {
    Err("FastTree inference is only available when compiled to wasm32".to_string())
}

/// wasm-bindgen: `buildTree(fasta, args) -> newick`.
#[wasm_bindgen(js_name = buildTree)]
pub fn build_tree_js(fasta: &str, args: Vec<JsValue>) -> Result<String, JsValue> {
    let mut owned = Vec::with_capacity(args.len());
    for value in &args {
        match value.as_string() {
            Some(s) => owned.push(s),
            None => return Err(JsValue::from_str("all options must be strings")),
        }
    }
    let borrowed: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    infer(fasta, &borrowed).map_err(|e| JsValue::from_str(&e))
}

/// wasm-bindgen convenience wrapper for nucleotide alignments.
#[wasm_bindgen(js_name = buildTreeNT)]
pub fn build_tree_nt_js(fasta: &str) -> Result<String, JsValue> {
    infer(fasta, &["-nt"]).map_err(|e| JsValue::from_str(&e))
}

/// wasm-bindgen convenience wrapper for protein alignments.
#[wasm_bindgen(js_name = buildTreeProtein)]
pub fn build_tree_protein_js(fasta: &str) -> Result<String, JsValue> {
    infer(fasta, &[]).map_err(|e| JsValue::from_str(&e))
}

/// Upstream FastTree version embedded in this crate.
#[wasm_bindgen(js_name = fastTreeVersion)]
pub fn fasttree_version() -> String {
    "2.1.11".to_string()
}

/// Floating-point precision of this build: "single" (FastTree's default) or
/// "double" (built with `--features double`, i.e. FastTree's -DUSE_DOUBLE).
#[wasm_bindgen(js_name = fastTreePrecision)]
pub fn fasttree_precision() -> String {
    if cfg!(feature = "double") { "double" } else { "single" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::validate_fasta;

    #[test]
    fn accepts_valid_alignment() {
        assert!(validate_fasta(">a\nACGT\n>b\nACGA\n", true).is_ok());
        assert!(validate_fasta(">a\nACDE\n>b\nACDF\n", false).is_ok());
    }

    #[test]
    fn rejects_bad_alignments() {
        assert!(validate_fasta("", true).is_err());
        assert!(validate_fasta(">only\nACGT\n", true).is_err());
        assert!(validate_fasta("ACGT\n>b\nACGT\n", true).is_err());
        assert!(validate_fasta(">a\nACG\n>b\nACGT\n", true).is_err());
        assert!(validate_fasta(">a\nACGT\n>b\nACQZ\n", true).is_err());
    }
}
