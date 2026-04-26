#ifndef OS_MATH_H
#define OS_MATH_H

#ifndef FP_NAN
#define FP_NAN 0
#endif
#ifndef FP_INFINITE
#define FP_INFINITE 1
#endif
#ifndef FP_NORMAL
#define FP_NORMAL 2
#endif
#ifndef FP_SUBNORMAL
#define FP_SUBNORMAL 3
#endif
#ifndef FP_ZERO
#define FP_ZERO 4
#endif
#ifndef INFINITY
#define INFINITY (__builtin_inff())
#endif
#ifndef NAN
#define NAN (__builtin_nanf(""))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef float float_t;
typedef double double_t;

double fabs(double x);
float fabsf(float x);
double sqrt(double x);
float sqrtf(float x);
double pow(double base, double exp);
float powf(float base, float exp);
double floor(double x);
double sin(double x);
double cos(double x);
double tan(double x);
double asin(double x);
double acos(double x);
double atan(double x);
double atan2(double y, double x);
double exp(double x);
double log(double x);
double log10(double x);
float sinf(float x);
float cosf(float x);
float tanf(float x);
float floorf(float x);
float ceilf(float x);
float roundf(float x);
long lroundf(float x);
double ceil(double x);
#define HUGE_VAL (__builtin_huge_val())
double fmod(double x, double y);
double hypot(double x, double y);
float fmodf(float x, float y);
float hypotf(float x, float y);
float acosf(float x);
float atan2f(float y, float x);
double ldexp(double x, int exp);
double frexp(double x, int *exp);
int isnan(double x);

#ifdef __cplusplus
}
#endif

#endif
