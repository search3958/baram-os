#ifndef BARAM_WCHAR_H
#define BARAM_WCHAR_H

#include <stddef.h>
#include <stdio.h>
#include <time.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef __cplusplus
typedef __WCHAR_TYPE__ wchar_t;
#endif

typedef unsigned int wint_t;

#ifndef WCHAR_MIN
#define WCHAR_MIN 0
#endif

#ifndef WCHAR_MAX
#define WCHAR_MAX ((wchar_t)-1)
#endif

#ifndef WEOF
#define WEOF ((wint_t)-1)
#endif

wchar_t* wcscpy(wchar_t* dst, const wchar_t* src);
wchar_t* wcsncpy(wchar_t* dst, const wchar_t* src, size_t n);
wchar_t* wcscat(wchar_t* dst, const wchar_t* src);
wchar_t* wcsncat(wchar_t* dst, const wchar_t* src, size_t n);
int wcscmp(const wchar_t* lhs, const wchar_t* rhs);
int wcsncmp(const wchar_t* lhs, const wchar_t* rhs, size_t n);
size_t wcslen(const wchar_t* s);
const wchar_t* wcschr(const wchar_t* s, wchar_t c);
const wchar_t* wcsrchr(const wchar_t* s, wchar_t c);
const wchar_t* wcspbrk(const wchar_t* s1, const wchar_t* s2);
const wchar_t* wcsstr(const wchar_t* haystack, const wchar_t* needle);
size_t wcscspn(const wchar_t* s1, const wchar_t* s2);
size_t wcsspn(const wchar_t* s1, const wchar_t* s2);
int wmemcmp(const wchar_t* s1, const wchar_t* s2, size_t n);
wchar_t* wmemcpy(wchar_t* dst, const wchar_t* src, size_t n);
wchar_t* wmemmove(wchar_t* dst, const wchar_t* src, size_t n);
wchar_t* wmemset(wchar_t* dst, wchar_t ch, size_t n);
const wchar_t* wmemchr(const wchar_t* s, wchar_t c, size_t n);
int swprintf(wchar_t* s, size_t n, const wchar_t* format, ...);
int vswprintf(wchar_t* s, size_t n, const wchar_t* format, void* arg);
double wcstod(const wchar_t* nptr, wchar_t** endptr);
long wcstol(const wchar_t* nptr, wchar_t** endptr, int base);
unsigned long wcstoul(const wchar_t* nptr, wchar_t** endptr, int base);
size_t wcsftime(wchar_t* s, size_t maxsize, const wchar_t* format,
                const struct tm* timeptr);
int fwide(FILE* stream, int mode);
wint_t btowc(int c);
int wctob(wint_t c);

#ifdef __cplusplus
}
#endif

#endif
