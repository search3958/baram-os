#ifndef _STRING_H_
#define _STRING_H_

#include <stddef.h>

// memset/memcpy の宣言のみ（実体は kernel.c に定義）
void *memset(void *dest, int val, size_t n);
void *memcpy(void *dest, const void *src, size_t n);
int memcmp(const void *s1, const void *s2, size_t n);

static inline int strcmp(const char *s1, const char *s2) {
  while (*s1 && (*s1 == *s2)) {
    s1++;
    s2++;
  }
  return *(unsigned char *)s1 - *(unsigned char *)s2;
}

static inline int strncmp(const char *s1, const char *s2, size_t n) {
  for (size_t i = 0; i < n; i++) {
    if (s1[i] != s2[i])
      return (unsigned char)s1[i] - (unsigned char)s2[i];
    if (s1[i] == '\0')
      return 0;
  }
  return 0;
}

static inline size_t strlen(const char *s) {
  size_t len = 0;
  while (s[len])
    len++;
  return len;
}

static inline char *strchr(const char *s, int c) {
  while (*s) {
    if (*s == (char)c)
      return (char *)s;
    s++;
  }
  return 0;
}

static inline char *strncpy(char *dest, const char *src, size_t n) {
  size_t i;
  for (i = 0; i < n && src[i] != '\0'; i++)
    dest[i] = src[i];
  for (; i < n; i++)
    dest[i] = '\0';
  return dest;
}

static inline char *strstr(const char *haystack, const char *needle) {
  if (!*needle)
    return (char *)haystack;
  for (; *haystack; haystack++) {
    const char *h = haystack;
    const char *n = needle;
    while (*h && *n && *h == *n) {
      h++;
      n++;
    }
    if (!*n)
      return (char *)haystack;
  }
  return 0;
}

static inline char *strcpy(char *dest, const char *src) {
  char *d = dest;
  while ((*d++ = *src++))
    ;
  return dest;
}

static inline int toupper(int c) {
  if (c >= 'a' && c <= 'z')
    return c - 32;
  return c;
}

static inline int tolower(int c) {
  if (c >= 'A' && c <= 'Z')
    return c + 32;
  return c;
}

#endif
