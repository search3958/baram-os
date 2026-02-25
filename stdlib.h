#ifndef OS_STDLIB_H
#define OS_STDLIB_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void *malloc(size_t size);
void free(void *ptr);
void *realloc(void *ptr, size_t size);
long strtol(const char *nptr, char **endptr, int base);
long long strtoll(const char *nptr, char **endptr, int base);
void qsort(void *base, size_t nmemb, size_t size,
           int (*compar)(const void *, const void *));

#ifdef __cplusplus
}
#endif

#endif
