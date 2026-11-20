#ifndef FT_COMPAT_UNISTD_H
#define FT_COMPAT_UNISTD_H

#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

int isatty(int fd);

#endif
