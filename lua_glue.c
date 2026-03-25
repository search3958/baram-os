#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <locale.h>
#include <time.h>
#include <errno.h>

// --- ctype.h ---
int isalpha(int c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'); }
int isdigit(int c) { return (c >= '0' && c <= '9'); }
int isalnum(int c) { return isalpha(c) || isdigit(c); }
int isspace(int c) { return (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v'); }
int isupper(int c) { return (c >= 'A' && c <= 'Z'); }
int islower(int c) { return (c >= 'a' && c <= 'z'); }
int isxdigit(int c) { return isdigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'); }
int iscntrl(int c) { return (c >= 0 && c < 32) || (c == 127); }
int isprint(int c) { return (c >= 32 && c < 127); }
int ispunct(int c) { return isprint(c) && !isalnum(c) && !isspace(c); }
int isgraph(int c) { return isprint(c) && c != ' '; }
int tolower(int c) { if (c >= 'A' && c <= 'Z') return c + ('a' - 'A'); return c; }
int toupper(int c) { if (c >= 'a' && c <= 'z') return c - ('a' - 'A'); return c; }

// --- math.h ---
#include <math.h>
double sin(double x) { return sinf((float)x); }
double cos(double x) { return cosf((float)x); }
double tan(double x) { return tanf((float)x); }
double floor(double x) { return floorf((float)x); }
double ceil(double x) { return ceilf((float)x); }
double fmod(double x, double y) { return fmodf((float)x, (float)y); }
double acos(double x) { return acosf((float)x); }
double atan2(double y, double x) { return atan2f((float)y, (float)x); }
double asin(double x) { return 0.0; } // Stub
double atan(double x) { return atan2f((float)x, 1.0f); }
double exp(double x) { return 0.0; } // Stub
double log(double x) { return 0.0; } // Stub
double log10(double x) { return 0.0; } // Stub

double ldexp(double x, int exp) {
    if (exp > 0) while (exp--) x *= 2.0;
    else if (exp < 0) while (exp++) x /= 2.0;
    return x;
}

double frexp(double x, int *exp) {
    if (x == 0.0) { *exp = 0; return 0.0; }
    int e = 0;
    double ax = (x < 0) ? -x : x;
    if (ax >= 1.0) {
        while (ax >= 1.0) { ax /= 2.0; e++; }
    } else if (ax < 0.5) {
        while (ax < 0.5) { ax *= 2.0; e--; }
    }
    *exp = e;
    return (x < 0) ? -ax : ax;
}

// --- locale.h ---
static struct lconv posix_lconv = {
    .decimal_point = ".",
    .thousands_sep = "",
    .grouping = "",
    .int_curr_symbol = "",
    .currency_symbol = "",
    .mon_decimal_point = "",
    .mon_thousands_sep = "",
    .mon_grouping = "",
    .positive_sign = "",
    .negative_sign = "",
    .int_frac_digits = 127,
    .frac_digits = 127,
    .p_cs_precedes = 127,
    .p_sep_by_space = 127,
    .n_cs_precedes = 127,
    .n_sep_by_space = 127,
    .p_sign_posn = 127,
    .n_sign_posn = 127
};

char *setlocale(int category, const char *locale) { return "C"; }
struct lconv *localeconv(void) { return &posix_lconv; }

// --- time.h ---
time_t time(time_t *t) {
    extern uint32_t timer_ticks;
    time_t res = timer_ticks / 100; // 100Hz
    if (t) *t = res;
    return res;
}

clock_t clock(void) {
    extern uint32_t timer_ticks;
    return (clock_t)timer_ticks;
}

double difftime(time_t time1, time_t time0) { return (double)(time1 - time0); }

char *asctime(const struct tm *tm) { return "Mon Jan  1 00:00:00 1970"; }
char *ctime(const time_t *timep) { return asctime(NULL); }
struct tm *gmtime(const time_t *timep) { return NULL; }
struct tm *localtime(const time_t *timep) { return NULL; }
time_t mktime(struct tm *tm) { return 0; }
size_t strftime(char *s, size_t max, const char *format, const struct tm *tm) {
    if (max > 0) s[0] = '\0';
    return 0;
}

// --- stdlib.h ---
void abort(void) {
    extern void set_w1_global(const char *key, const char *val);
    set_w1_global("--warpSystemLog", "LUA ABORT CALLED");
#ifdef __aarch64__
    while(1) { __asm__("wfi"); }
#else
    while(1) { __asm__("hlt"); }
#endif
}

void exit(int status) {
    abort();
}

char *getenv(const char *name) { return NULL; }
int system(const char *command) { return -1; }

int errno = 0;
FILE *stdin = NULL;
FILE *stdout = NULL;
FILE *stderr = NULL;

int abs(int x) { return x < 0 ? -x : x; }
double strtod(const char *nptr, char **endptr) {
    if (endptr) *endptr = (char *)nptr;
    return 0.0;
}
int strcoll(const char *a, const char *b) { return strcmp(a, b); }
char *fgets(char *s, int size, FILE *stream) { return NULL; }
FILE *freopen(const char *path, const char *mode, FILE *stream) { return NULL; }

// --- stdio.h stubs (already some in kernel.c but Lua needs more) ---
// --- stdio.h stubs (already some in kernel.c but Lua needs more) ---
int fflush(FILE *stream) { return 0; }
int remove(const char *filename) { return -1; }
int rename(const char *old, const char *new) { return -1; }
FILE *tmpfile(void) { return NULL; }
char *tmpnam(char *s) { return NULL; }
int setvbuf(FILE *stream, char *buf, int mode, size_t size) { return 0; }
void clearerr(FILE *stream) {}
int ferror(FILE *stream) { return 0; }
int feof(FILE *stream) { return 0; }
int ungetc(int c, FILE *stream) { return EOF; }
int getc(FILE *stream) { return EOF; }
int fputc(int c, FILE *stream) { return EOF; }
int fputs(const char *s, FILE *stream) { return EOF; }
int puts(const char *s) { return EOF; }

size_t fwrite(const void *ptr, size_t size, size_t nmemb, FILE *stream) {
    return 0;
}

// --- Lua Integration ---
#include "lua-master/lua.h"
#include "lua-master/lualib.h"
#include "lua-master/lauxlib.h"
#include "ui/warp1_engine.h"
#include "ui/warp_engine.h"
#include "fs.h"

static int l_warp_set_state(lua_State *L) {
    lua_getfield(L, LUA_REGISTRYINDEX, "WARP1_CTX");
    warp1_context_t *ctx = (warp1_context_t *)lua_touserdata(L, -1);
    const char *key = luaL_checkstring(L, 1);
    const char *val = luaL_checkstring(L, 2);
    if (ctx) warp1_context_set_state(ctx, key, val);
    return 0;
}

static int l_warp_add_node(lua_State *L) {
    lua_getfield(L, LUA_REGISTRYINDEX, "WARP1_CTX");
    warp1_context_t *ctx = (warp1_context_t *)lua_touserdata(L, -1);
    const char *parent_id = luaL_checkstring(L, 1);
    const char *tag = luaL_checkstring(L, 2);
    const char *id = luaL_checkstring(L, 3);
    if (ctx) warp1_context_add_node(ctx, parent_id, tag, id);
    return 0;
}

static int l_warp_set_attr(lua_State *L) {
    lua_getfield(L, LUA_REGISTRYINDEX, "WARP1_CTX");
    warp1_context_t *ctx = (warp1_context_t *)lua_touserdata(L, -1);
    const char *id = luaL_checkstring(L, 1);
    const char *key = luaL_checkstring(L, 2);
    const char *val = luaL_checkstring(L, 3);
    if (ctx) warp1_context_set_attr(ctx, id, key, val);
    return 0;
}

static int l_os_log(lua_State *L) {
    const char *msg = luaL_checkstring(L, 1);
    extern void set_w1_global(const char *key, const char *val);
    set_w1_global("--warpSystemLog", msg);
    return 0;
}

void run_lua_script(warp1_context_t *ctx, const char *filename) {
    lua_State *L = luaL_newstate();
    if (!L) return;
    luaL_openlibs(L);

    // Store context
    lua_pushlightuserdata(L, ctx);
    lua_setfield(L, LUA_REGISTRYINDEX, "WARP1_CTX");

    // Register BaramOS API
    lua_newtable(L);
    lua_pushcfunction(L, l_warp_set_state); lua_setfield(L, -2, "setState");
    lua_pushcfunction(L, l_warp_add_node);  lua_setfield(L, -2, "addNode");
    lua_pushcfunction(L, l_warp_set_attr);  lua_setfield(L, -2, "setAttr");
    lua_setglobal(L, "warp");

    lua_pushcfunction(L, l_os_log);
    lua_setglobal(L, "log");

    // Load and run
    uint32_t size = 0;
    void *code = fs_read_file(filename, &size);
    if (code) {
        if (luaL_loadbuffer(L, code, size, filename) == LUA_OK) {
            if (lua_pcall(L, 0, 0, 0) != LUA_OK) {
                const char *err = lua_tostring(L, -1);
                extern void set_w1_global(const char *key, const char *val);
                set_w1_global("--warpSystemLog", err);
            }
        } else {
            const char *err = lua_tostring(L, -1);
            extern void set_w1_global(const char *key, const char *val);
            set_w1_global("--warpSystemLog", err);
        }
        free(code);
    } else {
        extern void set_w1_global(const char *key, const char *val);
        char err[128] = "Lua: File not found: ";
        strlcat(err, filename, 127);
        set_w1_global("--warpSystemLog", err);
    }

    lua_close(L);
}

// Map root stdio functions to their kernel implementations if necessary
// But kernel.c implementations are usually enough if linked correctly.
