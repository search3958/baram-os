#include <stddef.h>
#ifndef HIDDEN_DEF
#define HIDDEN_DEF
#define hidden static
#endif
#ifndef LANGINFO_H
#define LANGINFO_H

#include "langinfo.h"

char *__nl_langinfo_l(nl_item, locale_t);

#endif
