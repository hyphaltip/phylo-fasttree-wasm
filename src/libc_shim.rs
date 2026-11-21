//! Minimal libc implementation used by the vendored FastTree C code.
//!
//! The C sources are compiled with `-nostdlib`; every libc symbol they need
//! is defined here (or in `vendor/src/ft_compat.c`). Keeping this in Rust
//! means there is one allocator, one linear memory, and no JS glue.

use core::sync::atomic::{AtomicI32, Ordering};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::ptr;

extern "C" {
    fn ft_take_stderr() -> *mut c_char;
}

const ALIGN: usize = 16;
const HEADER: usize = 16; // magic + payload size
const TRAILER: usize = 8; // canary (diagnostic only)
const CHUNK: usize = 1 << 20; // 1 MiB minimum chunk

#[repr(C)]
struct AllocHeader {
    magic: usize,
    size: usize, // payload size
}

const MAGIC: usize = 0x46545f4d; // "FT_M"

static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

#[no_mangle]
pub extern "C" fn ft_clear_error() {
    EXIT_CODE.store(0, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn ft_last_error() -> i32 {
    EXIT_CODE.load(Ordering::SeqCst)
}

/// Called by the C `exit()` stub. FastTree calls this when it rejects input or
/// arguments that the Rust pre-flight cannot anticipate (e.g. `-close` without
/// `-top`). Throwing a JS error here — instead of executing `unreachable` —
/// makes the failure visible to callers as a normal `catch (e)` while keeping
/// the WASM instance usable for subsequent calls.
#[no_mangle]
pub extern "C" fn ft_exit_hook(code: i32) {
    let status = if code == 0 { 1 } else { code };
    EXIT_CODE.store(status, Ordering::SeqCst);

    let mut msg = format!("FastTree exited with status {}", status);
    unsafe {
        let err = ft_take_stderr();
        if !err.is_null() {
            let detail = CStr::from_ptr(err).to_string_lossy();
            if !detail.trim().is_empty() {
                msg.push_str(": ");
                msg.push_str(detail.trim());
            }
            free(err as *mut c_void);
        }
    }
    wasm_bindgen::throw_str(&msg)
}

/* ------------------------------------------------------------------ */
/* Allocation                                                          */
/* ------------------------------------------------------------------ */
//
// FastTree performs an enormous number of small malloc/free/realloc calls and
// expects glibc's allocation semantics. Reimplementing a general heap in the
// shim was both slow and fragile: subtle differences in realloc() bookkeeping
// eventually corrupted the heap on long runs. Instead we use a bump allocator
// over chunks obtained from Rust's global allocator. `free()` is a no-op and
// `realloc()` copies into a fresh block. All C allocations are reclaimed at
// the start of each `infer()` call by rewinding the bump pointer. This is
// ideal for a short-lived, single-threaded inference call in WASM.

struct Chunk {
    ptr: *mut u8,
    len: usize,
}

unsafe impl Send for Chunk {}

static mut CHUNKS: Vec<Chunk> = Vec::new();
static mut CHUNK_IDX: usize = 0;
static mut OFFSET: usize = 0;

/// Reclaim all memory handed to C. Call only when no C pointers are live.
#[no_mangle]
pub extern "C" fn ft_reset_alloc() {
    unsafe {
        CHUNK_IDX = 0;
        OFFSET = 0;
    }
}

fn canary(size: usize) -> usize {
    MAGIC ^ size.rotate_left(17)
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }
    let need = match size.checked_add(HEADER + TRAILER) {
        Some(n) => (n + ALIGN - 1) & !(ALIGN - 1),
        None => return ptr::null_mut(),
    };

    if CHUNK_IDX >= CHUNKS.len() || OFFSET + need > CHUNKS[CHUNK_IDX].len {
        CHUNK_IDX = CHUNKS.len();
        OFFSET = 0;
        let len = need.max(CHUNK);
        let layout = match std::alloc::Layout::from_size_align(len, ALIGN) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        let p = std::alloc::alloc(layout);
        if p.is_null() {
            return ptr::null_mut();
        }
        CHUNKS.push(Chunk { ptr: p, len });
    }

    let base = CHUNKS[CHUNK_IDX].ptr.add(OFFSET);
    OFFSET += need;
    ptr::write(
        base as *mut AllocHeader,
        AllocHeader {
            magic: MAGIC,
            size,
        },
    );
    ptr::write(base.add(HEADER + size) as *mut usize, canary(size));
    base.add(HEADER) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    // Intentionally a no-op; memory is reclaimed by ft_reset_alloc().
    if p.is_null() {
        return;
    }
    let base = (p as *mut u8).sub(HEADER);
    let h = base as *mut AllocHeader;
    if (*h).magic == MAGIC {
        (*h).magic = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return malloc(size);
    }
    if size == 0 {
        free(p);
        return ptr::null_mut();
    }
    let base = (p as *mut u8).sub(HEADER);
    let h = base as *mut AllocHeader;
    if (*h).magic != MAGIC {
        return ptr::null_mut();
    }
    let old_size = (*h).size;
    let new = malloc(size) as *mut u8;
    if new.is_null() {
        return ptr::null_mut();
    }
    let n = old_size.min(size);
    ptr::copy_nonoverlapping(base.add(HEADER), new, n);
    (*h).magic = 0;
    new as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total = match nmemb.checked_mul(size) {
        Some(t) => t,
        None => return ptr::null_mut(),
    };
    let p = malloc(total) as *mut u8;
    if !p.is_null() {
        ptr::write_bytes(p, 0, total);
    }
    p as *mut c_void
}

/* ------------------------------------------------------------------ */
/* Memory and strings                                                  */
/* ------------------------------------------------------------------ */

/* memcpy/memmove/memset/memcmp are supplied by Rust's compiler_builtins.
   Defining them here and using ptr::copy_* under LTO creates a recursive
   call back into the exported symbol. */

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    let mut p = s as *const u8;
    while *p != 0 {
        p = p.add(1);
    }
    p.offset_from(s as *const u8) as usize
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(a: *const c_char, b: *const c_char) -> i32 {
    let mut pa = a as *const u8;
    let mut pb = b as *const u8;
    loop {
        let x = *pa;
        let y = *pb;
        if x != y {
            return x as i32 - y as i32;
        }
        if x == 0 {
            return 0;
        }
        pa = pa.add(1);
        pb = pb.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn strncmp(a: *const c_char, b: *const c_char, n: usize) -> i32 {
    let mut pa = a as *const u8;
    let mut pb = b as *const u8;
    for _ in 0..n {
        let x = *pa;
        let y = *pb;
        if x != y {
            return x as i32 - y as i32;
        }
        if x == 0 {
            return 0;
        }
        pa = pa.add(1);
        pb = pb.add(1);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut d = dst as *mut u8;
    let mut s = src as *const u8;
    loop {
        let c = *s;
        *d = c;
        if c == 0 {
            break;
        }
        d = d.add(1);
        s = s.add(1);
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut d = dst as *mut u8;
    let mut s = src as *const u8;
    for _ in 0..n {
        let c = *s;
        *d = c;
        if c == 0 {
            break;
        }
        d = d.add(1);
        s = s.add(1);
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut d = dst as *mut u8;
    while *d != 0 {
        d = d.add(1);
    }
    let mut s = src as *const u8;
    loop {
        let c = *s;
        *d = c;
        if c == 0 {
            break;
        }
        d = d.add(1);
        s = s.add(1);
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const c_char, c: i32) -> *mut c_char {
    let mut p = s as *const u8;
    loop {
        let x = *p;
        if x == c as u8 || x == 0 {
            return if x == c as u8 {
                p as *mut c_char
            } else {
                ptr::null_mut()
            };
        }
        p = p.add(1);
    }
}

/* ------------------------------------------------------------------ */
/* libm (FastTree's float build only uses these four)                  */
/* ------------------------------------------------------------------ */

#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    libm::log(x)
}

#[no_mangle]
pub extern "C" fn exp(x: f64) -> f64 {
    libm::exp(x)
}

#[no_mangle]
pub extern "C" fn sqrt(x: f64) -> f64 {
    libm::sqrt(x)
}

#[no_mangle]
pub extern "C" fn fabs(x: f64) -> f64 {
    libm::fabs(x)
}

/* ------------------------------------------------------------------ */
/* C float formatting helper for the tiny printf                       */
/* ------------------------------------------------------------------ */

fn strip_zeros(mut s: String) -> String {
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn normalize_exp(s: &str) -> String {
    // Rust: "1.2e5" / "1.2e-5"; C: "1.2e+05" / "1.2e-05"
    if let Some(pos) = s.find('e') {
        let (mant, exp) = s.split_at(pos);
        let e: i32 = exp[1..].parse().unwrap_or(0);
        format!("{}e{}{:02}", mant, if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        s.to_string()
    }
}

fn format_f64(value: f64, precision: i32, style: i32) -> String {
    let p = precision.max(0) as usize;

    match style {
        2 => {
            let s = format!("{:.*e}", p, value);
            let (mant, _) = s.split_at(s.find('e').unwrap_or(s.len()));
            format!("{}{}", strip_zeros(mant.to_string()), &s[mant.len()..])
        }
        1 => {
            // C %g: precision is significant digits (min 1).
            let p = if p == 0 { 1 } else { p } as i32;
            if value == 0.0 {
                return "0".to_string();
            }
            if !value.is_finite() {
                return value.to_string();
            }
            let exponent = libm::floor(libm::log10(libm::fabs(value))) as i32;
            if exponent < -4 || exponent >= p {
                let mut s = format!("{:.*e}", (p - 1).max(0) as usize, value);
                if let Some(pos) = s.find('e') {
                    let mant = strip_zeros(s[..pos].to_string());
                    s = format!("{}{}", mant, &s[pos..]);
                }
                normalize_exp(&s)
            } else {
                let decimals = (p - 1 - exponent).max(0) as usize;
                strip_zeros(format!("{:.*}", decimals, value))
            }
        }
        _ => format!("{:.*}", p, value),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ft_fmt_f64(
    out: *mut c_char,
    cap: usize,
    value: f64,
    precision: i32,
    style: i32,
) {
    if out.is_null() || cap == 0 {
        return;
    }
    let s = format_f64(value, precision, style);
    let n = s.len().min(cap - 1);
    ptr::copy_nonoverlapping(s.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
}
