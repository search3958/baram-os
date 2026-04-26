#ifndef BARAM_WCTYPE_H
#define BARAM_WCTYPE_H

#include <wchar.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef unsigned long wctrans_t;
typedef unsigned long wctype_t;

int iswalpha(wint_t c);
int iswalnum(wint_t c);
int iswspace(wint_t c);
int iswprint(wint_t c);
int iswdigit(wint_t c);
int iswxdigit(wint_t c);
wint_t towlower(wint_t c);
wint_t towupper(wint_t c);
wctrans_t wctrans(const char* name);
wctype_t wctype(const char* name);
wint_t towctrans(wint_t c, wctrans_t desc);
int iswctype(wint_t c, wctype_t desc);

#ifdef __cplusplus
}
#endif

#endif
