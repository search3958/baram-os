#ifndef OS_STDLIB_H
#define OS_STDLIB_H

#include <stddef.h>

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

#ifdef __cplusplus
extern "C" {
#endif

void *malloc(size_t size);
void free(void *ptr);
void *realloc(void *ptr, size_t size);
void abort(void);
void exit(int status);
char *getenv(const char *name);
int system(const char *command);
int abs(int j);
int atoi(const char *nptr);
long atol(const char *nptr);
long long atoll(const char *nptr);
long strtol(const char *nptr, char **endptr, int base);
long long strtoll(const char *nptr, char **endptr, int base);
double strtod(const char *nptr, char **endptr);
void abort(void);
void qsort(void *base, size_t nmemb, size_t size,
           int (*compar)(const void *, const void *));

#ifdef __cplusplus
}
#endif

#endif
