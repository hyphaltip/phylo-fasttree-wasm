#ifndef FT_COMPAT_STDIO_H
#define FT_COMPAT_STDIO_H

#include <stddef.h>

/* Opaque, in-memory stream. FastTree never inspects FILE internals. */
typedef struct FT_FILE {
    const char *in;        /* read buffer (borrowed) */
    size_t in_len;
    size_t in_pos;
    int unget;

    char *out;             /* write buffer (owned, malloc'd) */
    size_t out_len;
    size_t out_cap;

    int error;
} FILE;

extern FILE *stdin;
extern FILE *stdout;
extern FILE *stderr;

#define EOF (-1)
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

int fgetc(FILE *stream);
char *fgets(char *s, int n, FILE *stream);
int ungetc(int c, FILE *stream);
int ferror(FILE *stream);
int fseek(FILE *stream, long offset, int whence);
int fflush(FILE *stream);
FILE *fopen(const char *pathname, const char *mode);
int fclose(FILE *stream);
int fprintf(FILE *stream, const char *format, ...);
int printf(const char *format, ...);
int sprintf(char *str, const char *format, ...);
int fputs(const char *s, FILE *stream);
int sscanf(const char *str, const char *format, ...);
int fscanf(FILE *stream, const char *format, ...);

#endif
