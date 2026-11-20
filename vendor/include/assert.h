#ifndef FT_COMPAT_ASSERT_H
#define FT_COMPAT_ASSERT_H

/* Build with -DNDEBUG: assertions are disabled in the embedded build. */
#ifdef NDEBUG
#define assert(ignore) ((void)0)
#else
void __assert_fail(const char *assertion, const char *file, unsigned int line,
                   const char *function);
#define assert(expr) \
    ((expr) ? (void)0 : __assert_fail(#expr, __FILE__, __LINE__, __func__))
#endif

#endif
