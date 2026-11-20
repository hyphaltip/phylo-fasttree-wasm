#ifndef FT_COMPAT_MATH_H
#define FT_COMPAT_MATH_H

/* FastTree only calls these four libm functions (float build, no USE_DOUBLE). */
double log(double x);
double exp(double x);
double sqrt(double x);
double fabs(double x);

#define HUGE_VAL __builtin_inf()

#endif
