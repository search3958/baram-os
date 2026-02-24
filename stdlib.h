#ifndef _STDLIB_H_
#define _STDLIB_H_

#include <stddef.h>

static inline void *malloc(size_t size) {
  static unsigned char heap[4 * 1024 * 1024]; // 4MB Heap
  static size_t used = 0;
  // Align to 8 bytes
  size = (size + 7) & ~7;
  if (used + size > sizeof(heap))
    return 0;
  void *ptr = &heap[used];
  used += size;
  return ptr;
}

static inline void free(void *ptr) { (void)ptr; }

static inline void *realloc(void *ptr, size_t size) {
  if (!ptr)
    return malloc(size);
  void *new_ptr = malloc(size);
  if (!new_ptr)
    return 0;
  // 古いサイズが不明なため size バイトコピー（NanoSVGの用途では問題ない）
  unsigned char *s = (unsigned char *)ptr;
  unsigned char *d = (unsigned char *)new_ptr;
  for (size_t i = 0; i < size; i++)
    d[i] = s[i];
  return new_ptr;
}

static inline int abs(int x) { return x < 0 ? -x : x; }

static inline int atoi(const char *s) {
  int n = 0, sign = 1;
  while (*s == ' ' || *s == '\t')
    s++;
  if (*s == '-') {
    sign = -1;
    s++;
  } else if (*s == '+')
    s++;
  while (*s >= '0' && *s <= '9')
    n = n * 10 + (*s++ - '0');
  return sign * n;
}

static inline double atof(const char *s) {
  double res = 0.0, fact = 1.0;
  int dash = 0;
  while (*s == ' ' || *s == '\t')
    s++;
  if (*s == '-') {
    dash = 1;
    s++;
  }
  for (int point_seen = 0; *s; s++) {
    if (*s == '.') {
      point_seen = 1;
      continue;
    }
    int d = *s - '0';
    if (d >= 0 && d <= 9) {
      if (point_seen)
        fact /= 10.0;
      res = res * 10.0 + (double)d;
    } else
      break;
  }
  return dash ? -res * fact : res * fact;
}

// strtol: 文字列を long に変換 (base対応)
static inline long strtol(const char *str, char **endptr, int base) {
  const char *s = str;
  long result = 0;
  int sign = 1;

  while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r')
    s++;
  if (*s == '-') {
    sign = -1;
    s++;
  } else if (*s == '+')
    s++;

  if (base == 0) {
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
      base = 16;
      s += 2;
    } else if (s[0] == '0') {
      base = 8;
      s++;
    } else {
      base = 10;
    }
  } else if (base == 16 && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
    s += 2;
  }

  while (*s) {
    int digit;
    char c = *s;
    if (c >= '0' && c <= '9')
      digit = c - '0';
    else if (c >= 'a' && c <= 'z')
      digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'Z')
      digit = c - 'A' + 10;
    else
      break;
    if (digit >= base)
      break;
    result = result * base + digit;
    s++;
  }

  if (endptr)
    *endptr = (char *)s;
  return sign * result;
}

// strtoll: 文字列を long long に変換
static inline long long strtoll(const char *str, char **endptr, int base) {
  const char *s = str;
  long long result = 0;
  int sign = 1;

  while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r')
    s++;
  if (*s == '-') {
    sign = -1;
    s++;
  } else if (*s == '+')
    s++;

  if (base == 0) {
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
      base = 16;
      s += 2;
    } else if (s[0] == '0') {
      base = 8;
      s++;
    } else {
      base = 10;
    }
  } else if (base == 16 && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
    s += 2;
  }

  while (*s) {
    int digit;
    char c = *s;
    if (c >= '0' && c <= '9')
      digit = c - '0';
    else if (c >= 'a' && c <= 'z')
      digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'Z')
      digit = c - 'A' + 10;
    else
      break;
    if (digit >= base)
      break;
    result = result * base + digit;
    s++;
  }

  if (endptr)
    *endptr = (char *)s;
  return (long long)sign * result;
}

static inline void qsort(void *base, size_t nmemb, size_t size,
                         int (*compar)(const void *, const void *)) {
  // 挿入ソート（nanosvg内の qsort 呼び出し用）
  unsigned char *arr = (unsigned char *)base;
  unsigned char tmp[256]; // size <= 256 を想定
  for (size_t i = 1; i < nmemb; i++) {
    // tmp = arr[i]
    for (size_t b = 0; b < size; b++)
      tmp[b] = arr[i * size + b];
    long long j = (long long)i - 1;
    while (j >= 0 && compar(arr + j * size, tmp) > 0) {
      for (size_t b = 0; b < size; b++)
        arr[(j + 1) * size + b] = arr[j * size + b];
      j--;
    }
    for (size_t b = 0; b < size; b++)
      arr[(j + 1) * size + b] = tmp[b];
  }
}

#endif
