/*
 * Minimal in-wasm libc compatibility layer for FastTree.
 *
 * Everything FastTree needs is provided here or by the Rust shim
 * (src/libc_shim.rs). No filesystem, no libc, no JS glue.
 */
#include "ft_compat.h"

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <sys/time.h>

/* ------------------------------------------------------------------ */
/* In-memory streams                                                   */
/* ------------------------------------------------------------------ */

static FILE ft_stdin_mem;
static FILE ft_stdout_mem;
static FILE ft_stderr_mem;

FILE *stdin = &ft_stdin_mem;
FILE *stdout = &ft_stdout_mem;
FILE *stderr = &ft_stderr_mem;

#define MAX_WRITE_FILES 8
static FILE *write_files[MAX_WRITE_FILES];
static int n_write_files = 0;

void ft_set_input(const char *p, size_t n)
{
    ft_stdin_mem.in = p;
    ft_stdin_mem.in_len = n;
    ft_stdin_mem.in_pos = 0;
    ft_stdin_mem.unget = -1;
    ft_stdin_mem.error = 0;
}

void ft_reset_output(void)
{
    int i;
    /* These buffers live in the C bump allocator, which is rewound at the
       start of every build. Never keep stale pointers across calls. */
    free(ft_stdout_mem.out);
    ft_stdout_mem.out = NULL;
    ft_stdout_mem.out_len = 0;
    ft_stdout_mem.out_cap = 0;

    free(ft_stderr_mem.out);
    ft_stderr_mem.out = NULL;
    ft_stderr_mem.out_len = 0;
    ft_stderr_mem.out_cap = 0;

    for (i = 0; i < n_write_files; i++) {
        free(write_files[i]->out);
        free(write_files[i]);
    }
    n_write_files = 0;
}

static char *take_buffer(FILE *f)
{
    size_t n = f->out_len;
    char *copy = (char *)malloc(n + 1);
    if (copy == NULL)
        return NULL;
    if (n && f->out)
        memcpy(copy, f->out, n);
    copy[n] = 0;
    f->out_len = 0;
    if (f->out) f->out[0] = 0;
    return copy;
}

char *ft_take_output(void)
{
    size_t total = ft_stdout_mem.out_len;
    int i;
    for (i = 0; i < n_write_files; i++)
        total += write_files[i]->out_len;

    char *copy = (char *)malloc(total + 1);
    if (copy == NULL)
        return NULL;

    size_t pos = 0;
    if (ft_stdout_mem.out_len) {
        memcpy(copy, ft_stdout_mem.out, ft_stdout_mem.out_len);
        pos += ft_stdout_mem.out_len;
    }
    for (i = 0; i < n_write_files; i++) {
        FILE *f = write_files[i];
        if (f->out_len) {
            memcpy(copy + pos, f->out, f->out_len);
            pos += f->out_len;
        }
        free(f->out);
        free(f);
    }
    n_write_files = 0;
    copy[pos] = 0;
    ft_stdout_mem.out_len = 0;
    if (ft_stdout_mem.out) ft_stdout_mem.out[0] = 0;
    return copy;
}

char *ft_take_stderr(void)
{
    return take_buffer(&ft_stderr_mem);
}

static int ft_append(FILE *f, const char *s, size_t n)
{
    if (n == 0)
        return 0;
    size_t need = f->out_len + n + 1;
    if (need > f->out_cap) {
        size_t cap = f->out_cap ? f->out_cap : 256;
        while (cap < need) {
            if (cap > (size_t)-1 / 2) { cap = need; break; }
            cap *= 2;
        }
        char *p = (char *)realloc(f->out, cap);
        if (p == NULL) {
            f->error = 1;
            return -1;
        }
        f->out = p;
        f->out_cap = cap;
    }
    memcpy(f->out + f->out_len, s, n);
    f->out_len += n;
    f->out[f->out_len] = 0;
    return (int)n;
}

int fgetc(FILE *f)
{
    if (f->unget >= 0) {
        int c = f->unget;
        f->unget = -1;
        return c;
    }
    if (f->in == NULL || f->in_pos >= f->in_len)
        return EOF;
    return (unsigned char)f->in[f->in_pos++];
}

int ungetc(int c, FILE *f)
{
    if (c == EOF)
        return EOF;
    f->unget = c;
    return c;
}

char *fgets(char *s, int n, FILE *f)
{
    if (n <= 0)
        return NULL;
    int i = 0;
    while (i < n - 1) {
        int c = fgetc(f);
        if (c == EOF) {
            if (i == 0) return NULL;
            break;
        }
        s[i++] = (char)c;
        if (c == '\n') break;
    }
    s[i] = 0;
    return s;
}

int ferror(FILE *f)
{
    return f->error;
}

int fseek(FILE *f, long offset, int whence)
{
    if (f->in == NULL)
        return -1;
    long base = 0;
    if (whence == SEEK_CUR) base = (long)f->in_pos;
    else if (whence == SEEK_END) base = (long)f->in_len;
    long pos = base + offset;
    if (pos < 0 || (size_t)pos > f->in_len)
        return -1;
    f->in_pos = (size_t)pos;
    f->unget = -1;
    return 0;
}

int fflush(FILE *f)
{
    (void)f;
    return 0;
}

FILE *fopen(const char *path, const char *mode)
{
    (void)path;
    if (mode && mode[0] == 'r') {
        FILE *f = (FILE *)malloc(sizeof(FILE));
        if (!f) return NULL;
        /* A valid but empty stream: matrix/transition-file options are
           rejected by the Rust API before we ever get here. */
        memset(f, 0, sizeof(*f));
        f->unget = -1;
        return f;
    }
    if (n_write_files < MAX_WRITE_FILES) {
        FILE *f = (FILE *)malloc(sizeof(FILE));
        if (!f) return NULL;
        memset(f, 0, sizeof(*f));
        f->unget = -1;
        write_files[n_write_files++] = f;
        return f;
    }
    return NULL;
}

int fclose(FILE *f)
{
    if (f == NULL || f == stdin || f == stdout || f == stderr)
        return 0;
    /* Write-file buffers are collected by ft_take_output(). */
    return 0;
}

int fputs(const char *s, FILE *f)
{
    return ft_append(f, s, strlen(s));
}

int printf(const char *format, ...)
{
    va_list ap;
    va_start(ap, format);
    int r = ft_vfprintf(stdout, format, ap);
    va_end(ap);
    return r;
}

int fprintf(FILE *f, const char *format, ...)
{
    va_list ap;
    va_start(ap, format);
    int r = ft_vfprintf(f, format, ap);
    va_end(ap);
    return r;
}

int sprintf(char *str, const char *format, ...)
{
    FILE tmp;
    memset(&tmp, 0, sizeof(tmp));
    tmp.unget = -1;
    va_list ap;
    va_start(ap, format);
    ft_vfprintf(&tmp, format, ap);
    va_end(ap);
    size_t n = tmp.out_len;
    memcpy(str, tmp.out ? tmp.out : "", n + 1);
    free(tmp.out);
    return (int)n;
}

/* ------------------------------------------------------------------ */
/* Tiny printf                                                         */
/* ------------------------------------------------------------------ */

int ft_vfprintf(FILE *f, const char *fmt, va_list ap)
{
    int written = 0;
    const char *p = fmt;
    if (p == NULL)
        return 0;

    while (*p) {
        if (*p != '%') {
            const char *start = p;
            while (*p && *p != '%') p++;
            written += ft_append(f, start, (size_t)(p - start));
            continue;
        }

        p++; /* consume '%' */
        if (*p == '%') {
            written += ft_append(f, "%", 1);
            p++;
            continue;
        }

        int left = 0, zero = 0, plus = 0, space = 0;
        for (;;) {
            if (*p == '-') { left = 1; p++; }
            else if (*p == '0') { zero = 1; p++; }
            else if (*p == '+') { plus = 1; p++; }
            else if (*p == ' ') { space = 1; p++; }
            else if (*p == '#') { p++; }
            else break;
        }

        int width = 0;
        while (*p >= '0' && *p <= '9') { width = width * 10 + (*p - '0'); p++; }

        int precision = -1;
        if (*p == '.') {
            p++;
            precision = 0;
            while (*p >= '0' && *p <= '9') { precision = precision * 10 + (*p - '0'); p++; }
        }

        int is_long = 0;
        while (*p == 'l' || *p == 'h' || *p == 'L') {
            if (*p == 'l') is_long = 1;
            p++;
        }

        char conv = *p ? *p : '\0';
        if (*p) p++;

        char buf[128];
        const char *out = buf;
        size_t len = 0;
        int pad_zero = zero && !left;

        switch (conv) {
        case 'd': case 'i': {
            long v = is_long ? va_arg(ap, long) : (long)va_arg(ap, int);
            char digits[64];
            size_t nd = 0;
            unsigned long mag;
            int neg = v < 0;
            if (neg) mag = (unsigned long)(-(v + 1)) + 1UL;
            else mag = (unsigned long)v;
            do { digits[nd++] = (char)('0' + (mag % 10)); mag /= 10; } while (mag);
            int min_digits = precision > 0 ? precision : 1;
            while (nd < (size_t)min_digits) digits[nd++] = '0';
            if (neg) buf[len++] = '-';
            else if (plus) buf[len++] = '+';
            else if (space) buf[len++] = ' ';
            while (nd) buf[len++] = digits[--nd];
            if (precision >= 0) pad_zero = 0; /* C: precision overrides '0' flag */
            break;
        }
        case 'u': {
            unsigned long v = is_long ? va_arg(ap, unsigned long)
                                      : (unsigned long)va_arg(ap, unsigned int);
            char digits[64];
            size_t nd = 0;
            do { digits[nd++] = (char)('0' + (v % 10)); v /= 10; } while (v);
            int min_digits = precision > 0 ? precision : 1;
            while (nd < (size_t)min_digits) digits[nd++] = '0';
            while (nd) buf[len++] = digits[--nd];
            if (precision >= 0) pad_zero = 0;
            break;
        }
        case 'c': {
            int c = va_arg(ap, int);
            buf[len++] = (char)c;
            break;
        }
        case 's': {
            char *s = va_arg(ap, char *);
            if (s == NULL) s = (char *)"(null)";
            size_t slen = strlen(s);
            if (precision >= 0 && (size_t)precision < slen) slen = (size_t)precision;
            /* Emit straight from the argument: sequence names can be longer
               than buf, and truncating them would corrupt the Newick labels. */
            out = s;
            len = slen;
            pad_zero = 0;
            break;
        }
        case 'f': case 'F': case 'g': case 'G': case 'e': case 'E': {
            double v = va_arg(ap, double);
            int prec = precision >= 0 ? precision : 6;
            if (conv == 'g' || conv == 'G') {
                if (prec == 0) prec = 1;
                ft_fmt_f64(buf, sizeof(buf), v, prec, 1);
            } else if (conv == 'e' || conv == 'E') {
                ft_fmt_f64(buf, sizeof(buf), v, prec, 2);
            } else {
                ft_fmt_f64(buf, sizeof(buf), v, prec, 0);
            }
            len = strlen(buf);
            pad_zero = 0; /* Rust emits leading zeros already */
            break;
        }
        default: {
            buf[len++] = '%';
            if (conv) buf[len++] = conv;
            break;
        }
        }

        int pad = width - (int)len;
        if (pad > 0) {
            char padc = pad_zero ? '0' : ' ';
            if (!left) {
                if (pad_zero && (buf[0] == '-' || buf[0] == '+')) {
                    written += ft_append(f, buf, 1);
                    len--;
                    memmove(buf, buf + 1, len);
                }
                for (int i = 0; i < pad; i++) written += ft_append(f, &padc, 1);
            }
            written += ft_append(f, out, len);
            if (left) for (int i = 0; i < pad; i++) written += ft_append(f, &padc, 1);
        } else {
            written += ft_append(f, out, len);
        }
    }
    return written;
}

/* ------------------------------------------------------------------ */
/* Numeric conversions                                                 */
/* ------------------------------------------------------------------ */

static const char *skip_ws(const char *s)
{
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r' || *s == '\f' || *s == '\v') s++;
    return s;
}

long ft_c_atol(const char *s)
{
    s = skip_ws(s);
    int neg = 0;
    if (*s == '+' || *s == '-') { neg = (*s == '-'); s++; }
    long v = 0;
    while (*s >= '0' && *s <= '9') { v = v * 10 + (*s - '0'); s++; }
    return neg ? -v : v;
}

double ft_c_atof(const char *s)
{
    s = skip_ws(s);
    int neg = 0;
    if (*s == '+' || *s == '-') { neg = (*s == '-'); s++; }

    double v = 0.0;
    while (*s >= '0' && *s <= '9') { v = v * 10.0 + (double)(*s - '0'); s++; }
    if (*s == '.') {
        s++;
        double scale = 0.1;
        while (*s >= '0' && *s <= '9') { v += (double)(*s - '0') * scale; scale *= 0.1; s++; }
    }
    if (*s == 'e' || *s == 'E') {
        s++;
        int eneg = 0;
        if (*s == '+' || *s == '-') { eneg = (*s == '-'); s++; }
        int e = 0;
        while (*s >= '0' && *s <= '9') { e = e * 10 + (*s - '0'); s++; }
        while (e-- > 0) { if (eneg) v /= 10.0; else v *= 10.0; }
    }
    return neg ? -v : v;
}

int atoi(const char *s) { return (int)ft_c_atol(s); }
long atol(const char *s) { return ft_c_atol(s); }
double atof(const char *s) { return ft_c_atof(s); }

int abs(int n) { return n < 0 ? -n : n; }
long labs(long n) { return n < 0 ? -n : n; }

/* ------------------------------------------------------------------ */
/* Strings, ctype, sort                                                */
/* ------------------------------------------------------------------ */

static char *tok_save = NULL;

char *strtok(char *s, const char *delim)
{
    if (s != NULL) tok_save = s;
    if (tok_save == NULL) return NULL;
    char *start = tok_save;
    while (*start && strchr(delim, *start)) start++;
    if (*start == 0) { tok_save = NULL; return NULL; }
    char *end = start;
    while (*end && !strchr(delim, *end)) end++;
    if (*end) {
        *end = 0;
        tok_save = end + 1;
    } else {
        tok_save = NULL;
    }
    return start;
}

int isdigit(int c) { return c >= '0' && c <= '9'; }
int isalpha(int c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'); }
int isalnum(int c) { return isalpha(c) || isdigit(c); }
int isspace(int c) { return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v'; }
int isupper(int c) { return c >= 'A' && c <= 'Z'; }
int islower(int c) { return c >= 'a' && c <= 'z'; }
int toupper(int c) { return islower(c) ? c - 'a' + 'A' : c; }
int tolower(int c) { return isupper(c) ? c - 'A' + 'a' : c; }

/* Stable merge sort, matching glibc qsort's behavior on equal elements
   (FastTree's comparators can tie, and tie order can affect the topology). */
static void ft_msort(char *a, size_t n, size_t size,
                     int (*compar)(const void *, const void *), char *tmp)
{
    if (n < 2)
        return;
    size_t mid = n / 2;
    ft_msort(a, mid, size, compar, tmp);
    ft_msort(a + mid * size, n - mid, size, compar, tmp);

    size_t i = 0, j = mid, k = 0;
    while (i < mid && j < n) {
        const char *left = a + i * size;
        const char *right = a + j * size;
        if (compar(left, right) <= 0) {
            memcpy(tmp + k * size, left, size);
            i++;
        } else {
            memcpy(tmp + k * size, right, size);
            j++;
        }
        k++;
    }
    while (i < mid) { memcpy(tmp + k * size, a + i * size, size); i++; k++; }
    while (j < n) { memcpy(tmp + k * size, a + j * size, size); j++; k++; }
    memcpy(a, tmp, n * size);
}

void qsort(void *base, size_t n, size_t size,
           int (*compar)(const void *, const void *))
{
    if (n < 2 || size == 0 || compar == NULL)
        return;
    char *tmp = (char *)malloc(n * size);
    if (!tmp)
        return;
    ft_msort((char *)base, n, size, compar, tmp);
    free(tmp);
}

/* ------------------------------------------------------------------ */
/* OS stubs                                                            */
/* ------------------------------------------------------------------ */

int isatty(int fd) { (void)fd; return 0; }

int gettimeofday(struct timeval *tv, void *tz)
{
    (void)tz;
    if (tv) { tv->tv_sec = 0; tv->tv_usec = 0; }
    return 0;
}

/* Minimal stubs: these code paths are only used by -matrix / -trans,
   which the Rust API rejects before calling FastTree. */
int sscanf(const char *str, const char *format, ...)
{
    (void)str; (void)format;
    return EOF;
}
int fscanf(FILE *stream, const char *format, ...)
{
    (void)stream; (void)format;
    return EOF;
}

void exit(int code)
{
    ft_exit_hook(code);
    __builtin_unreachable();
}

/* ------------------------------------------------------------------ */
/* Public C entry point                                                */
/* ------------------------------------------------------------------ */

char *fasttree_build(const char *fasta, size_t len, int argc, char **argv)
{
    ft_set_input(fasta, len);
    ft_reset_output();
    ft_reset_fasttree_globals();
    return fasttree_main_impl(argc, argv);
}
