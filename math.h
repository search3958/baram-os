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
float sinf(float x);
float cosf(float x);
float tanf(float x);
float floorf(float x);
float ceilf(float x);
float roundf(float x);
float fmodf(float x, float y);
float acosf(float x);
float atan2f(float y, float x);
int isnan(double x);

#ifdef __cplusplus
}
#endif

#endif
