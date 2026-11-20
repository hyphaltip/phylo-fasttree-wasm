#ifndef FT_COMPAT_STDLIB_H
#define FT_COMPAT_STDLIB_H

#include <stddef.h>

void *malloc(size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);
void exit(int status);

int abs(int n);
long labs(long n);

int atoi(const char *nptr);
long atol(const char *nptr);
double atof(const char *nptr);

void qsort(void *base, size_t nmemb, size_t size,
           int (*compar)(const void *, const void *));

#endif
