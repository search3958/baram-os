#ifndef _STDIO_H_
#define _STDIO_H_

#include <stdarg.h>
#include <stddef.h>

// カーネル環境ではファイルI/Oは使用不可
// NanoSVGの nsvgParseFromFile を無効化するために FILE型と関連マクロを定義
typedef int FILE;

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

// ダミー実装（呼び出されないが定義が必要）
static inline FILE *fopen(const char *path, const char *mode) {
  (void)path;
  (void)mode;
  return 0;
}
static inline int fclose(FILE *fp) {
  (void)fp;
  return 0;
}
static inline int fseek(FILE *fp, long offset, int whence) {
  (void)fp;
  (void)offset;
  (void)whence;
  return -1;
}
static inline long ftell(FILE *fp) {
  (void)fp;
  return -1;
}
static inline size_t fread(void *ptr, size_t size, size_t count, FILE *fp) {
  (void)ptr;
  (void)size;
  (void)count;
  (void)fp;
  return 0;
}

static inline int printf(const char *format, ...) {
  (void)format;
  return 0;
}
static inline int sprintf(char *str, const char *format, ...) {
  (void)str;
  (void)format;
  return 0;
}

static inline int sscanf(const char *str, const char *format, ...) {
  (void)str;
  (void)format;
  return 0;
}

#endif
