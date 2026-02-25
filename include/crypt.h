#include <stddef.h>
#ifndef HIDDEN_DEF
#define HIDDEN_DEF
#define hidden static
#endif
#ifndef CRYPT_H
#define CRYPT_H

#include "crypt.h"

#include <features.h>

hidden char *__crypt_r(const char *, const char *, struct crypt_data *);

hidden char *__crypt_des(const char *, const char *, char *);
hidden char *__crypt_md5(const char *, const char *, char *);
hidden char *__crypt_blowfish(const char *, const char *, char *);
hidden char *__crypt_sha256(const char *, const char *, char *);
hidden char *__crypt_sha512(const char *, const char *, char *);

#endif
