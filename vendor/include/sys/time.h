#ifndef FT_COMPAT_SYS_TIME_H
#define FT_COMPAT_SYS_TIME_H

struct timeval {
    long tv_sec;
    long tv_usec;
};

int gettimeofday(struct timeval *tv, void *tz);

#endif
