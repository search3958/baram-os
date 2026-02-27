#ifndef STDLIB_H
#define STDLIB_H
#include <stddef.h>

typedef struct { int quot, rem; } div_t;
typedef struct { long quot, rem; } ldiv_t;

int abs(int);
int atoi(const char *);
long atol(const char *);
long long atoll(const char *);
div_t div(int, int);

void *malloc(size_t size);
void free(void *ptr);
void *realloc(void *ptr, size_t size);

#endif
