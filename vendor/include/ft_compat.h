#ifndef FT_COMPAT_H
#define FT_COMPAT_H

#include <stddef.h>
#include <stdio.h>
#include <stdarg.h>

/* Implemented by the in-memory stdio shim. */
void ft_set_input(const char *p, size_t n);
void ft_reset_output(void);
char *ft_take_output(void);
char *ft_take_stderr(void);

/* Implemented by ft_reset.c (FastTree file-scope globals). */
void ft_reset_fasttree_globals(void);

/* Implemented by FastTree.c after the embedded edits. */
char *fasttree_main_impl(int argc, char **argv);

/* Public C entry point implemented in ft_compat.c. */
char *fasttree_build(const char *fasta, size_t len, int argc, char **argv);

/* Formatter (used by fprintf/sprintf in the compat layer). */
int ft_vfprintf(FILE *stream, const char *fmt, va_list ap);

/* Rust hooks. */
void ft_exit_hook(int code);
void ft_clear_error(void);
int ft_last_error(void);
void ft_fmt_f64(char *out, size_t cap, double value, int precision, int style);

#endif
