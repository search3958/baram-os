#ifndef OS_STDIO_H
#define OS_STDIO_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct FILE {
  int dummy;
} FILE;

#ifndef SEEK_SET
#define SEEK_SET 0
#endif
#ifndef SEEK_CUR
#define SEEK_CUR 1
#endif
#ifndef SEEK_END
#define SEEK_END 2
#endif

FILE *fopen(const char *path, const char *mode);
int fclose(FILE *stream);
size_t fread(void *ptr, size_t size, size_t nmemb, FILE *stream);
int fseek(FILE *stream, long offset, int whence);
long ftell(FILE *stream);
int fprintf(FILE *stream, const char *format, ...);
int sscanf(const char *str, const char *format, ...);

#ifdef __cplusplus
}
#endif

#endif
