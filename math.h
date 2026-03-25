#ifndef OS_MATH_H
#define OS_MATH_H

#ifdef __cplusplus
extern "C" {
#endif

double fabs(double x);
float fabsf(float x);
double sqrt(double x);
float sqrtf(float x);
double pow(double base, double exp);
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
double ceil(double x);
#define HUGE_VAL (__builtin_huge_val())
double fmod(double x, double y);
float fmodf(float x, float y);
float acosf(float x);
float atan2f(float y, float x);
double ldexp(double x, int exp);
double frexp(double x, int *exp);
int isnan(double x);

#ifdef __cplusplus
}
#endif

#endif
