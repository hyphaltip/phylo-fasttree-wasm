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
/// Every C allocation is prefixed by this header and linked into a list of
/// live blocks, so `ft_reset_alloc()` can reclaim anything a run leaked
/// (including everything left behind when FastTree `exit()`s mid-run).
#[repr(C)]
struct AllocHeader {
    magic: usize,
    size: usize, // payload size
    prev: *mut AllocHeader,
    next: *mut AllocHeader,
}
const HEADER: usize = (core::mem::size_of::<AllocHeader>() + ALIGN - 1) & !(ALIGN - 1);

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
// malloc/free/realloc forward to Rust's global allocator (dlmalloc on
// wasm32), so freed memory is actually reused. FastTree frees most of what it
// allocates, so peak memory tracks native FastTree instead of the cumulative
// total of every allocation ever made. Live blocks are kept on an intrusive
// doubly-linked list; `ft_reset_alloc()` frees whatever is still live, which
// keeps the instance usable after an aborted (thrown) run.

static mut LIVE: *mut AllocHeader = ptr::null_mut();

#[inline]
unsafe fn layout_for(size: usize) -> Option<std::alloc::Layout> {
    let total = size.checked_add(HEADER)?;
    std::alloc::Layout::from_size_align(total, ALIGN).ok()
}

#[inline]
unsafe fn link(h: *mut AllocHeader) {
    (*h).prev = ptr::null_mut();
    (*h).next = LIVE;
    if !LIVE.is_null() {
        (*LIVE).prev = h;
    }
    LIVE = h;
}

#[inline]
unsafe fn unlink(h: *mut AllocHeader) {
    if (*h).prev.is_null() {
        LIVE = (*h).next;
    } else {
        (*(*h).prev).next = (*h).next;
    }
    if !(*h).next.is_null() {
        (*(*h).next).prev = (*h).prev;
    }
}

/// Free every C allocation that is still live. Call only when no C pointers
/// are in use (i.e. between FastTree runs).
#[no_mangle]
pub extern "C" fn ft_reset_alloc() {
    unsafe {
        let mut h = LIVE;
        while !h.is_null() {
            let next = (*h).next;
            let layout = layout_for((*h).size).unwrap();
            (*h).magic = 0;
            std::alloc::dealloc(h as *mut u8, layout);
            h = next;
        }
        LIVE = ptr::null_mut();
    }
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }
    let layout = match layout_for(size) {
        Some(l) => l,
        None => return ptr::null_mut(),
    };
    let base = std::alloc::alloc(layout);
    if base.is_null() {
        return ptr::null_mut();
    }
    let h = base as *mut AllocHeader;
    (*h).magic = MAGIC;
    (*h).size = size;
    link(h);
    base.add(HEADER) as *mut c_void
}

#[inline]
unsafe fn header_of(p: *mut c_void) -> Option<*mut AllocHeader> {
    let h = (p as *mut u8).sub(HEADER) as *mut AllocHeader;
    if (*h).magic == MAGIC { Some(h) } else { None }
}

#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    if let Some(h) = header_of(p) {
        unlink(h);
        (*h).magic = 0;
        std::alloc::dealloc(h as *mut u8, layout_for((*h).size).unwrap());
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
    let h = match header_of(p) {
        Some(h) => h,
        None => return ptr::null_mut(),
    };
    let new_total = match size.checked_add(HEADER) {
        Some(n) => n,
        None => return ptr::null_mut(),
    };
    unlink(h);
    let old_layout = layout_for((*h).size).unwrap();
    let nb = std::alloc::realloc(h as *mut u8, old_layout, new_total);
    if nb.is_null() {
        link(h); // original block is untouched on failure
        return ptr::null_mut();
    }
    let nh = nb as *mut AllocHeader;
    (*nh).size = size;
    link(nh);
    nb.add(HEADER) as *mut c_void
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
