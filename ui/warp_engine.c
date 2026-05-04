//こちらはclassic warp-古い方です。
#include "warp_engine.h"
#include <stddef.h>
#include <stdlib.h>
#include <math.h>

#define WARP_MAX_DRAW_OPS 2048

static int warp_strlen(const char *s) {
  int n = 0;
  if (!s)
    return 0;
  while (s[n])
    n++;
  return n;
}

static char *warp_strcpy(char *dest, const char *src) {
  char *d = dest;
  while ((*d++ = *src++))
    ;
  return dest;
}

static char *warp_strncpy(char *dest, const char *src, size_t n) {
  size_t i;
  for (i = 0; i < n && src[i] != '\0'; i++)
    dest[i] = src[i];
  for (; i < n; i++)
    dest[i] = '\0';
  return dest;
}

char *warp_strcat(char *dest, const char *src) {
  char *d = dest;
  while (*d)
    d++;
  while ((*d++ = *src++))
    ;
  return dest;
}

char *warp_strncat(char *dest, const char *src, size_t n) {
  if (n <= 0)
    return dest;
  char *d = dest;
  while (*d)
    d++;
  size_t i;
  for (i = 0; i < n && src[i] != '\0'; i++)
    *d++ = src[i];
  *d = '\0';
  return dest;
}

static int warp_strcmp(const char *s1, const char *s2) {
  while (*s1 && (*s1 == *s2)) {
    s1++;
    s2++;
  }
  return *(const unsigned char *)s1 - *(const unsigned char *)s2;
}

static int warp_tolower(int c) {
  if (c >= 'A' && c <= 'Z')
    return c + ('a' - 'A');
  return c;
}

static int warp_strcasecmp(const char *s1, const char *s2) {
  while (*s1 && (warp_tolower((unsigned char)*s1) == warp_tolower((unsigned char)*s2))) {
    s1++;
    s2++;
  }
  return warp_tolower((unsigned char)*s1) - warp_tolower((unsigned char)*s2);
}

static int warp_strncmp(const char *s1, const char *s2, size_t n) {
  if (n == 0)
    return 0;
  do {
    if (*s1 != *s2++)
      return *(const unsigned char *)s1 - *(const unsigned char *)--s2;
    if (*s1++ == 0)
      break;
  } while (--n != 0);
  return 0;
}

static int warp_strncasecmp(const char *s1, const char *s2, size_t n) {
  if (n == 0)
    return 0;
  do {
    if (warp_tolower((unsigned char)*s1) != warp_tolower((unsigned char)*s2))
      return warp_tolower((unsigned char)*s1) - warp_tolower((unsigned char)*s2);
    if (*s1++ == 0)
      break;
    s2++;
  } while (--n != 0);
  return 0;
}

static char *warp_strchr(const char *s, int c) {
  while (*s != (char)c) {
    if (!*s++)
      return NULL;
  }
  return (char *)s;
}

static char *warp_strstr(const char *haystack, const char *needle) {
  if (!*needle) return (char *)haystack;
  for (; *haystack; haystack++) {
    if (*haystack == *needle) {
      const char *h, *n;
      for (h = haystack, n = needle; *h && *n && *h == *n; h++, n++);
      if (!*n) return (char *)haystack;
    }
  }
  return NULL;
}

static long warp_strtol(const char *s) {
  long res = 0;
  int sign = 1;
  if (!s)
    return 0;
  const char *p = s;
  while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')
    p++;
  if (*p == '-') {
    sign = -1;
    p++;
  }
  while (*p >= '0' && *p <= '9') {
    if (res > 200000000)
      break;
    res = res * 10 + (*p - '0');
    p++;
  }
  return res * sign;
}

static char *append_uint(char *p, unsigned int v) {
  char tmp[16];
  int n = 0;
  if (v == 0) {
    *p++ = '0';
    *p = '\0';
    return p;
  }
  while (v > 0 && n < (int)sizeof(tmp)) {
    tmp[n++] = (char)('0' + (v % 10));
    v /= 10;
  }
  while (n-- > 0) {
    *p++ = tmp[n];
  }
  *p = '\0';
  return p;
}

char *append_int(char *p, int v) {
  unsigned int uv;
  if (v < 0) {
    *p++ = '-';
    uv = (unsigned int)-(v + 1) + 1;
  } else {
    uv = (unsigned int)v;
  }
  return append_uint(p, uv);
}

char *append_hex8(char *p, uint8_t v) {
  const char *hex = "0123456789ABCDEF";
  *p++ = hex[(v >> 4) & 0xF];
  *p++ = hex[v & 0xF];
  *p = '\0';
  return p;
}

char *warp_stpcpy(char *dest, const char *src) {
  while ((*dest = *src)) {
    dest++;
    src++;
  }
  return dest;
}

char *append_fixed3(char *p, float v) {
  int i = (int)v;
  float f_part = v - (float)i;
  if (f_part < 0)
    f_part = -f_part;
  int f = (int)(f_part * 1000.0f + 0.5f);
  p = append_int(p, i);
  *p++ = '.';
  if (f < 100)
    *p++ = '0';
  if (f < 10)
    *p++ = '0';
  p = append_int(p, f);
  return p;
}

static void warp_memset(void *s, int c, size_t n) {
  unsigned char *p = s;
  while (n--)
    *p++ = (unsigned char)c;
}

static const char *get_color_hex(const char *name) {
  if (!name || name[0] == '\0')
    return NULL;
  if (warp_strcmp(name, "yellow") == 0)
    return "#fbc02d";
  if (warp_strcmp(name, "red") == 0)
    return "#b3261e";
  if (warp_strcmp(name, "blue") == 0)
    return "#0a56d0";
  if (warp_strcmp(name, "gray") == 0)
    return "#808080";
  if (warp_strcmp(name, "black") == 0)
    return "#000000";
  if (name[0] == '#')
    return name;
  return NULL;
}

typedef struct warp_attr {
  char key[32];
  char value[512];
} warp_attr_t;

typedef struct warp_node {
  char tag[32];
  warp_attr_t attrs[16];
  int attrs_count;
  char event_oneclick[512];
  char event_longpress[512];
  struct warp_node *children[64];
  int children_count;
  int x, y, w, h;
  int is_dynamic;

  // For Diffing
  int prev_x, prev_y, prev_w, prev_h;
  int is_dirty;
  uint32_t state_hash;
  uint32_t prev_state_hash;
} warp_node_t;

typedef struct {
  char type[16]; // "if", "elseIf"
  char condition[256];
  char actions[1024];
} script_block_t;

typedef struct {
  char name[64];
  script_block_t blocks[MAX_SCRIPT_BLOCKS];
  int block_count;
} script_t;

typedef struct {
  char id[64];
  int visible;
} visibility_t;

typedef struct {
  char target_id[64];
  warp_node_t *nodes[16];
  int node_count;
} dynamic_nodes_t;

typedef enum { TK_WORD, TK_STR, TK_PUNCT, TK_AT, TK_EOF } tk_type;
typedef struct {
  tk_type type;
  char val[512];
} token_t;

typedef struct { char id[64]; int token_index; } screen_info_t;

struct warp_context {
  struct {
    char key[64];
    char val[512];
  } *state;
  int state_count;
  int state_capacity;

  char current_screen[64];
  char parsed_screen_id[64];
  screen_info_t *screens;
  int screens_count;
  int screens_capacity;
  int screens_count_scanned;

  visibility_t *visibility;
  int visibility_count;
  int visibility_capacity;

  warp_node_t *nodes;
  int nodes_count;
  int nodes_capacity;
  warp_node_t *root_node;
  warp_node_t **root_nodes;
  int root_nodes_count;
  int root_nodes_capacity;

  script_t *scripts;
  int scripts_count;
  int scripts_capacity;

  dynamic_nodes_t *dynamic_nodes;
  int dynamic_nodes_count;
  int dynamic_nodes_capacity;

  const char *src_ptr;
  token_t *tokens;
  int token_count;
  int token_capacity;
  int token_pos;

  struct {
    int x, y;
    char text[512];
    uint32_t color;
    float size;
  } *texts;
  int texts_count;
  int texts_capacity;

  char *svg_output;
  int svg_capacity;
  warp_draw_op_t *draw_ops;
  int draw_ops_count;
  int draw_ops_capacity;
  int engine_dirty;
  char engine_status[128];
  char node_svg_buf[4096];
  int mouse_x, mouse_y;
  int win_w, win_h;
  int active_slider_idx;

  // Screen management (separate SVG per screen)
  char screen_ids[MAX_SCREENS][64];
  int screen_content_heights[MAX_SCREENS];
  float screen_scroll_ys[MAX_SCREENS];
  int screen_count;
};

// 前方宣言
static void set_state(warp_context_t *ctx, const char *key, const char *val);
static const char *get_state(warp_context_t *ctx, const char *key);
static void parse_current_screen_classic(warp_context_t *ctx);
static void skip_block_classic(warp_context_t *ctx);

// Capacity management
static void ensure_state_capacity(warp_context_t *ctx) {
  if (ctx->state_count >= ctx->state_capacity) {
    ctx->state_capacity *= 2;
    ctx->state = realloc(ctx->state, ctx->state_capacity * sizeof(*ctx->state));
  }
}

static void ensure_nodes_capacity(warp_context_t *ctx) {
  if (ctx->nodes_count >= ctx->nodes_capacity) {
    ctx->nodes_capacity *= 2;
    ctx->nodes = realloc(ctx->nodes, ctx->nodes_capacity * sizeof(warp_node_t));
  }
}

static void ensure_tokens_capacity(warp_context_t *ctx) {
  if (ctx->token_count >= ctx->token_capacity) {
    ctx->token_capacity *= 2;
    ctx->tokens = realloc(ctx->tokens, ctx->token_capacity * sizeof(token_t));
  }
}

static void ensure_texts_capacity(warp_context_t *ctx) {
  if (ctx->texts_count >= ctx->texts_capacity) {
    ctx->texts_capacity *= 2;
    ctx->texts = realloc(ctx->texts, ctx->texts_capacity * sizeof(*ctx->texts));
  }
}

static void ensure_draw_ops_capacity(warp_context_t *ctx) {
  if (ctx->draw_ops_count >= ctx->draw_ops_capacity) {
    ctx->draw_ops_capacity *= 2;
    ctx->draw_ops = realloc(ctx->draw_ops, ctx->draw_ops_capacity * sizeof(warp_draw_op_t));
  }
}


static void set_state(warp_context_t *ctx, const char *key, const char *val) {
  if (warp_strcasecmp(key, "_currentScreen") == 0) {
    warp_strncpy(ctx->current_screen, val, 63);
    return;
  }
  for (int i = 0; i < ctx->state_count; i++) {
    if (warp_strcasecmp(ctx->state[i].key, key) == 0) {
      warp_strncpy(ctx->state[i].val, val, 511);
      return;
    }
  }
  ensure_state_capacity(ctx);
  warp_strncpy(ctx->state[ctx->state_count].key, key, 63);
  warp_strncpy(ctx->state[ctx->state_count].val, val, 511);
  ctx->state_count++;
}

static const char *get_state(warp_context_t *ctx, const char *key) {
  if (warp_strcasecmp(key, "_currentScreen") == 0)
    return ctx->current_screen;
  for (int i = 0; i < ctx->state_count; i++) {
    if (warp_strcasecmp(ctx->state[i].key, key) == 0)
      return ctx->state[i].val;
  }
  return "";
}

static void set_visibility(warp_context_t *ctx, const char *id, int visible) {
  for (int i = 0; i < ctx->visibility_count; i++) {
    if (warp_strcmp(ctx->visibility[i].id, id) == 0) {
      ctx->visibility[i].visible = visible;
      return;
    }
  }
  if (ctx->visibility_count < MAX_VARS) {
    warp_strncpy(ctx->visibility[ctx->visibility_count].id, id, 63);
    ctx->visibility[ctx->visibility_count].visible = visible;
    ctx->visibility_count++;
  }
}

static int get_visibility(warp_context_t *ctx, const char *id) {
  if (!id || id[0] == '\0')
    return 1;
  for (int i = 0; i < ctx->visibility_count; i++) {
    if (warp_strcmp(ctx->visibility[i].id, id) == 0)
      return ctx->visibility[i].visible;
  }
  return 1;
}

static warp_node_t *alloc_node(warp_context_t *ctx) {
  ensure_nodes_capacity(ctx);
  warp_node_t *n = &ctx->nodes[ctx->nodes_count++];
  warp_memset(n, 0, sizeof(warp_node_t));
  return n;
}

static void set_attr(warp_node_t *node, const char *key, const char *value) {
  for (int i = 0; i < node->attrs_count; i++) {
    if (warp_strcmp(node->attrs[i].key, key) == 0) {
      warp_strncpy(node->attrs[i].value, value, 511);
      return;
    }
  }
  if (node->attrs_count < 16) {
    warp_strncpy(node->attrs[node->attrs_count].key, key, 31);
    warp_strncpy(node->attrs[node->attrs_count].value, value, 511);
    node->attrs_count++;
  }
}

static const char *get_attr(warp_node_t *node, const char *key) {
  for (int i = 0; i < node->attrs_count; i++) {
    if (warp_strcmp(node->attrs[i].key, key) == 0)
      return node->attrs[i].value;
  }
  return "";
}

static void eval_expr(warp_context_t *ctx, const char *expr, char *out, int max_len) {
  out[0] = '\0';
  const char *p = expr;
  while (*p) {
    if (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t') {
      p++;
      continue;
    }
    if (*p == '+') {
      p++;
      continue;
    }
    if (*p == '\"') {
      p++;
      while (*p && *p != '\"') {
        int len = warp_strlen(out);
        if (len < max_len - 1) {
          if (*p == '\\') {
            p++;
            if (*p == 'n')
              out[len] = '\n';
            else if (*p == '\"')
              out[len] = '\"';
            else if (*p == '\'')
              out[len] = '\'';
            else if (*p == '\\')
              out[len] = '\\';
            else if (*p == ' ')
              out[len] = ' ';
            else if (*p == '(')
              out[len] = '(';
            else if (*p == ')')
              out[len] = ')';
            else if (*p == ':')
              out[len] = ':';
            else
              out[len] = *p;
            out[len + 1] = '\0';
            if (*p)
              p++;
            continue;
          }
          out[len] = *p;
          out[len + 1] = '\0';
        }
        p++;
      }
      if (*p == '\"')
        p++;
    } else if (warp_strncmp(p, "--", 2) == 0 || warp_strncmp(p, "~~", 2) == 0) {
      char var[64];
      int i = 0;
      while (*p && *p != '\"' && *p != '+' && *p != ' ' && *p != ')' &&
             *p != ',' && i < 63)
        var[i++] = *p++;
      var[i] = '\0';
      const char *val = get_state(ctx, var);
      int remaining = max_len - warp_strlen(out) - 1;
      if (remaining > 0)
        warp_strncat(out, val, remaining);
    } else {
      int len = warp_strlen(out);
      if (len < max_len - 1) {
        out[len] = *p;
        out[len + 1] = '\0';
      }
      p++;
    }
  }
}

static void eval_attr(warp_context_t *ctx, warp_node_t *node, const char *key, char *buf, int size) {
  const char *val = get_attr(node, key);
  if (val[0] == '\0') {
    buf[0] = '\0';
    return;
  }
  eval_expr(ctx, val, buf, size);
}

static void get_switch_state_key(warp_node_t *node, char *buf, int size) {
  const char *raw = get_attr(node, "status");
  if (!raw[0])
    raw = get_attr(node, "output");
  if (!raw[0] || size <= 0) {
    if (size > 0)
      buf[0] = '\0';
    return;
  }

  while (*raw == ' ' || *raw == '\t' || *raw == '\n' || *raw == '\r')
    raw++;

  if (*raw == '(') {
    warp_strncpy(buf, raw + 1, size - 1);
    buf[size - 1] = '\0';
    char *end = warp_strchr(buf, ')');
    if (end)
      *end = '\0';
  } else {
    warp_strncpy(buf, raw, size - 1);
    buf[size - 1] = '\0';
  }

  int len = warp_strlen(buf);
  while (len > 0 && (buf[len - 1] == ' ' || buf[len - 1] == '\t' ||
                     buf[len - 1] == '\n' || buf[len - 1] == '\r')) {
    buf[--len] = '\0';
  }
}

static void get_slider_state_key(warp_node_t *node, char *buf, int size) {
  const char *raw = get_attr(node, "status");
  if (!raw[0])
    raw = get_attr(node, "output");
  if (!raw[0] || size <= 0) {
    if (size > 0)
      buf[0] = '\0';
    return;
  }

  while (*raw == ' ' || *raw == '\t' || *raw == '\n' || *raw == '\r')
    raw++;

  if (*raw == '(') {
    warp_strncpy(buf, raw + 1, size - 1);
    buf[size - 1] = '\0';
    char *end = warp_strchr(buf, ')');
    if (end)
      *end = '\0';
  } else {
    warp_strncpy(buf, raw, size - 1);
    buf[size - 1] = '\0';
  }

  int len = warp_strlen(buf);
  while (len > 0 && (buf[len - 1] == ' ' || buf[len - 1] == '\t' ||
                     buf[len - 1] == '\n' || buf[len - 1] == '\r')) {
    buf[--len] = '\0';
  }
}

static int update_slider_value(warp_context_t *ctx, warp_node_t *node, int x) {
  char out_var[128];
  get_slider_state_key(node, out_var, sizeof(out_var));
  if (!out_var[0] || node->w <= 0) return 0;

  int val = (x - node->x) * 100 / node->w;
  if (val < 0) val = 0;
  if (val > 100) val = 100;

  char val_str[16];
  append_int(val_str, val);
  const char *current = get_state(ctx, out_var);
  if (warp_strcmp(current, val_str) == 0) return 0;

  set_state(ctx, out_var, val_str);
  ctx->engine_dirty = 1;
  return 1;
}

static token_t next_token(warp_context_t *ctx) {
  token_t tk;
  tk.type = TK_EOF;
  tk.val[0] = 0;
  while (*ctx->src_ptr == ' ' || *ctx->src_ptr == '\n' || *ctx->src_ptr == '\r' ||
         *ctx->src_ptr == '\t')
    ctx->src_ptr++;
  if (!*ctx->src_ptr)
    return tk;
  if (*ctx->src_ptr == '@') {
    tk.type = TK_AT;
    tk.val[0] = *ctx->src_ptr++;
    tk.val[1] = 0;
    return tk;
  }
  if (*ctx->src_ptr == '\"' || *ctx->src_ptr == '\'') {
    char quote = *ctx->src_ptr++;
    int i = 0;
    tk.type = TK_STR;
    while (*ctx->src_ptr && *ctx->src_ptr != quote && i < 511) {
      if (*ctx->src_ptr == '\\') {
        tk.val[i++] = *ctx->src_ptr++;
        if (*ctx->src_ptr)
          tk.val[i++] = *ctx->src_ptr++;
        continue;
      }
      tk.val[i++] = *ctx->src_ptr++;
    }
    if (*ctx->src_ptr == quote)
      ctx->src_ptr++;
    tk.val[i] = 0;
    return tk;
  }
  if (*ctx->src_ptr == '(' || *ctx->src_ptr == ')' || *ctx->src_ptr == ':' ||
      *ctx->src_ptr == '=' || *ctx->src_ptr == '+' || *ctx->src_ptr == ',') {
    tk.type = TK_PUNCT;
    tk.val[0] = *ctx->src_ptr++;
    tk.val[1] = 0;
    return tk;
  }
  tk.type = TK_WORD;
  int i = 0;
  while (*ctx->src_ptr && (unsigned char)*ctx->src_ptr > 32 && *ctx->src_ptr != '(' &&
         *ctx->src_ptr != ')' && *ctx->src_ptr != ':' && *ctx->src_ptr != '=' &&
         *ctx->src_ptr != '+' && *ctx->src_ptr != ',' && *ctx->src_ptr != '\"' &&
         *ctx->src_ptr != '\'' && i < 511) {
    tk.val[i++] = *ctx->src_ptr++;
  }
  tk.val[i] = 0;
  return tk;
}

static warp_node_t *parse_node(warp_context_t *ctx);

static void parse_script(warp_context_t *ctx) {
  if (ctx->token_pos >= ctx->token_count || ctx->tokens[ctx->token_pos].type != TK_AT)
    return;
  ctx->token_pos++; // skip @
  if (ctx->token_pos >= ctx->token_count || ctx->tokens[ctx->token_pos].type != TK_WORD)
    return;
  if (ctx->scripts_count >= MAX_SCRIPTS)
    return;
  script_t *s = &ctx->scripts[ctx->scripts_count++];
  warp_memset(s, 0, sizeof(script_t));
  warp_strncpy(s->name, ctx->tokens[ctx->token_pos].val, 63);
  ctx->token_pos++;
  if (ctx->token_pos >= ctx->token_count || ctx->tokens[ctx->token_pos].val[0] != '(')
    return;
  ctx->token_pos++; // skip (
  while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != ')') {
    if (warp_strcmp(ctx->tokens[ctx->token_pos].val, "if") == 0 ||
        warp_strcmp(ctx->tokens[ctx->token_pos].val, "elseIf") == 0) {
      if (s->block_count >= MAX_SCRIPT_BLOCKS)
        break;
      script_block_t *b = &s->blocks[s->block_count++];
      warp_strncpy(b->type, ctx->tokens[ctx->token_pos].val, 15);
      ctx->token_pos++;
      if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ':')
        ctx->token_pos++;
      char cond[256] = "";
      while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != '(') {
        warp_strncat(cond, ctx->tokens[ctx->token_pos].val,
                     255 - warp_strlen(cond));
        ctx->token_pos++;
      }
      warp_strncpy(b->condition, cond, 255);
      if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == '(') {
        ctx->token_pos++; // skip (
        char actions[1024] = "";
        int paren = 1;
        while (ctx->token_pos < ctx->token_count && paren > 0) {
          if (ctx->tokens[ctx->token_pos].val[0] == '(')
            paren++;
          else if (ctx->tokens[ctx->token_pos].val[0] == ')') {
            paren--;
            if (paren == 0)
              break;
          }
          if (ctx->tokens[ctx->token_pos].type == TK_STR)
            warp_strncat(actions, "\"", 1023 - warp_strlen(actions));
          warp_strncat(actions, ctx->tokens[ctx->token_pos].val,
                       1023 - warp_strlen(actions));
          if (ctx->tokens[ctx->token_pos].type == TK_STR)
            warp_strncat(actions, "\"", 1023 - warp_strlen(actions));
          ctx->token_pos++;
        }
        warp_strncpy(b->actions, actions, 1023);
        if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ')')
          ctx->token_pos++;
      }
    } else {
      ctx->token_pos++;
    }
  }
  if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ')')
    ctx->token_pos++;
}

static warp_node_t *parse_node(warp_context_t *ctx) {
  if (ctx->token_pos >= ctx->token_count)
    return NULL;
  if (ctx->tokens[ctx->token_pos].type == TK_AT) {
    parse_script(ctx);
    return NULL;
  }
  token_t t = ctx->tokens[ctx->token_pos];
  if (ctx->token_pos + 1 < ctx->token_count &&
      ctx->tokens[ctx->token_pos + 1].val[0] == '(') {
    warp_node_t *node = alloc_node(ctx);
    if (!node)
      return NULL;
    warp_strcpy(node->tag, t.val);
    ctx->token_pos += 2;
    while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != ')') {
      token_t current_tk = ctx->tokens[ctx->token_pos];
      if (ctx->token_pos + 1 < ctx->token_count &&
          ctx->tokens[ctx->token_pos + 1].val[0] == '(') {
        warp_node_t *child = parse_node(ctx);
        if (child && node->children_count < 64)
          node->children[node->children_count++] = child;
        continue;
      }
      if (ctx->token_pos + 1 < ctx->token_count &&
          ctx->tokens[ctx->token_pos + 1].val[0] == ':') {
        char key[64];
        warp_strcpy(key, current_tk.val);
        ctx->token_pos += 2;
        char expr[512];
        expr[0] = '\0';
        int paren = 0;
        while (ctx->token_pos < ctx->token_count) {
          if (paren == 0) {
            if (ctx->tokens[ctx->token_pos].val[0] == ')')
              break;
            if (ctx->token_pos + 1 < ctx->token_count &&
                ctx->tokens[ctx->token_pos + 1].val[0] == '(') {
              const char *name = ctx->tokens[ctx->token_pos].val;
              int is_reserved =
                  (warp_strcmp(name, "reset") == 0 ||
                   warp_strcmp(name, "calc") == 0 ||
                   warp_strcmp(name, "script") == 0 ||
                   warp_strcmp(name, "add") == 0 ||
                   warp_strcmp(name, "del") == 0 ||
                   warp_strcmp(name, "clr") == 0 ||
                   warp_strcmp(name, "show") == 0 ||
                   warp_strcmp(name, "hide") == 0 ||
                   warp_strncmp(name, "setScreen", 9) == 0);
              if (!is_reserved)
                break;
            }
            if (ctx->token_pos + 1 < ctx->token_count &&
                ctx->tokens[ctx->token_pos + 1].val[0] == ':')
              break;
            if (ctx->tokens[ctx->token_pos].val[0] == ',') {
              if (warp_strcmp(key, "oneClick") != 0 &&
                  warp_strcmp(key, "onClick") != 0 &&
                  warp_strcmp(key, "longPress") != 0) {
                ctx->token_pos++;
                break;
              }
            }
          }
          if (ctx->tokens[ctx->token_pos].val[0] == '(')
            paren++;
          else if (ctx->tokens[ctx->token_pos].val[0] == ')')
            paren--;

          if (ctx->tokens[ctx->token_pos].type == TK_STR) {
            warp_strncat(expr, "\"", 511 - warp_strlen(expr));
            warp_strncat(expr, ctx->tokens[ctx->token_pos].val,
                         511 - warp_strlen(expr));
            warp_strncat(expr, "\"", 511 - warp_strlen(expr));
          } else
            warp_strncat(expr, ctx->tokens[ctx->token_pos].val,
                         511 - warp_strlen(expr));
          ctx->token_pos++;
        }
        if (warp_strcmp(key, "oneClick") == 0 ||
            warp_strcmp(key, "onClick") == 0)
          warp_strncpy(node->event_oneclick, expr, 511);
        else if (warp_strcmp(key, "longPress") == 0)
          warp_strncpy(node->event_longpress, expr, 511);
        else
          set_attr(node, key, expr);
        continue;
      }
      ctx->token_pos++;
    }
    if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ')')
      ctx->token_pos++;
    return node;
  }
  ctx->token_pos++;
  return NULL;
}

static void init_state_from_ast(warp_context_t *ctx, warp_node_t *node) {
  if (!node)
    return;
  for (int i = 0; i < node->attrs_count; i++) {
    if (node->attrs[i].key[0] == '-' && node->attrs[i].key[1] == '-') {
      char val[512];
      eval_expr(ctx, node->attrs[i].value, val, sizeof(val));
      set_state(ctx, node->attrs[i].key, val);
    }
  }
  for (int i = 0; i < node->children_count; i++)
    init_state_from_ast(ctx, node->children[i]);
}

static uint32_t warp_hash(const char *s, uint32_t h) {
  while (*s)
    h = h * 31 + (uint32_t)*s++;
  return h;
}

static uint32_t calc_node_state_hash(warp_context_t *ctx, warp_node_t *node) {
  uint32_t h = 0;
  h = warp_hash(node->tag, h);
  for (int i = 0; i < node->attrs_count; i++) {
    h = warp_hash(node->attrs[i].key, h);
    h = warp_hash(node->attrs[i].value, h);
    // Evaluate expressions because they might depend on global state
    char val[512];
    eval_expr(ctx, node->attrs[i].value, val, sizeof(val));
    h = warp_hash(val, h);
  }
  const char *id = get_attr(node, "id");
  h = warp_hash(id, h);
  int visible = get_visibility(ctx, id);
  h = h * 31 + (uint32_t)visible;
  return h;
}

static int layout_node(warp_context_t *ctx, warp_node_t *node, int px, int py, int limit_w) {
  if (!node)
    return 0;

  // Save previous state for diffing
  node->prev_x = node->x;
  node->prev_y = node->y;
  node->prev_w = node->w;
  node->prev_h = node->h;
  node->prev_state_hash = node->state_hash;
  node->state_hash = calc_node_state_hash(ctx, node);

  const char *id = get_attr(node, "id");
  if (!get_visibility(ctx, id)) {
    node->w = 0;
    node->h = 0;
    if (node->prev_w != 0 || node->prev_h != 0) {
        node->is_dirty = 1;
        ctx->engine_dirty = 1;
    }
    return 0;
  }

  node->x = px;
  node->y = py;
  node->w = limit_w;
  int cy = py;

  const char *dark_val = get_state(ctx, "~~main/dark");
  int is_dark = (warp_strcmp(dark_val, "true") == 0);

  if (warp_strcmp(node->tag, "screen") == 0) {
    const char *has_header = get_state(ctx, "~~internal/has_header");
    int padding = (warp_strcmp(has_header, "true") == 0) ? 60 : 16;
    cy = py + padding;
    for (int i = 0; i < node->children_count; i++) {
      if (warp_strcmp(node->children[i]->tag, "Header") == 0) {
        // Still layout header to update its state/children, but it won't affect cy
        layout_node(ctx, node->children[i], px, py, limit_w);
        continue;
      }
      cy += layout_node(ctx, node->children[i], px + 24, cy, limit_w - 48) + 12;
    }
    // Add 16px bottom padding
    node->h = cy - py + 4; // cy already has +12 from the last element, so +4 makes it +16 total
    if (node->h < ctx->win_h) node->h = ctx->win_h;
  } else if (warp_strcmp(node->tag, "Header") == 0) {
    node->h = 0;
    // Header actions: position from right to left
    int action_x = limit_w - 12;
    for (int i = 0; i < node->children_count; i++) {
      warp_node_t *act = node->children[i];
      // Save current texts_count to find text entry later
      int tc_before = ctx->texts_count;
      
      // Use regular layout logic with a large enough limit to avoid clipping
      layout_node(ctx, act, 0, 8, 200); 
      
      // Shift x position to right-aligned action_x
      action_x -= act->w;
      int shift = action_x - act->x;
      act->x = action_x;
      
      // Adjust text position if it was added
      if (ctx->texts_count > tc_before) {
        ctx->texts[tc_before].x += shift;
      }
      
      action_x -= 8;
    }
  } else if (warp_strcmp(node->tag, "card") == 0) {
    cy += 12;
    char title[128];
    eval_attr(ctx, node, "text", title, sizeof(title));
    if (title[0] != '\0' && ctx->texts_count < MAX_TEXTS) {
      ctx->texts[ctx->texts_count].x = px + 24;
      ctx->texts[ctx->texts_count].y = cy + 4;
      warp_strcpy(ctx->texts[ctx->texts_count].text, title);
      const char *c_prop = get_attr(node, "color");
      const char *hex = get_color_hex(c_prop);
      ctx->texts[ctx->texts_count].color = hex ? 0xFF000000 : (is_dark ? 0xFFEEEEEE : 0xFF121212);
      ctx->texts[ctx->texts_count].size = 20;
      ctx->texts_count++;
      cy += 36;
    }
    int cx = px + 24;
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(ctx, node->children[i], cx, cy, limit_w - 48) + 8;
    if (id[0] != '\0') {
      for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
        if (warp_strcmp(ctx->dynamic_nodes[i].target_id, id) == 0) {
          for (int j = 0; j < ctx->dynamic_nodes[i].node_count; j++) {
            cy += layout_node(ctx, ctx->dynamic_nodes[i].nodes[j], cx, cy,
                               limit_w - 48) + 8;
          }
        }
      }
    }
    node->h = cy - py + 12;
    if (node->h < 40)
      node->h = 40;
  } else if (warp_strcmp(node->tag, "text") == 0) {
    char text[512];
    eval_attr(ctx, node, "text", text, sizeof(text));
    if (text[0] != '\0' && ctx->texts_count < MAX_TEXTS) {
      ctx->texts[ctx->texts_count].x = px + 4;
      ctx->texts[ctx->texts_count].y = py + 4;
      warp_strcpy(ctx->texts[ctx->texts_count].text, text);
      const char *c_prop = get_attr(node, "color");
      const char *hex = get_color_hex(c_prop);
      ctx->texts[ctx->texts_count].color = hex ? 0xFF000000 : (is_dark ? 0xFFCCCCCC : 0xFF333333);
      ctx->texts[ctx->texts_count].size = 18;
      ctx->texts_count++;
    }
    int lines = 1;
    for (int i = 0; text[i]; i++)
      if (text[i] == '\n')
        lines++;
    int len = warp_strlen(text);
    if (len / 40 + 1 > lines)
      lines = len / 40 + 1;
    node->h = lines * 24;
  } else if (warp_strcmp(node->tag, "button") == 0 ||
             warp_strcmp(node->tag, "tonalButton") == 0) {
    char text[128];
    eval_attr(ctx, node, "text", text, sizeof(text));
    
    char width_prop[32];
    eval_attr(ctx, node, "width", width_prop, sizeof(width_prop));
    if (warp_strcmp(width_prop, "max") == 0) {
      node->w = limit_w;
    } else {
      int text_w = measure_ttf_width(text, 16.0f);
      node->w = text_w + 32;
      if (node->w < 70) node->w = 70;
      if (node->w > limit_w) node->w = limit_w; // Ensure we don't exceed given limit
    }
    node->h = 40;
    
    if (text[0] != '\0' && ctx->texts_count < MAX_TEXTS) {
      int text_w = measure_ttf_width(text, 16.0f);
      ctx->texts[ctx->texts_count].x = px + (node->w - text_w) / 2;
      ctx->texts[ctx->texts_count].y = py + 10;
      warp_strcpy(ctx->texts[ctx->texts_count].text, text);
      if (warp_strcmp(node->tag, "tonalButton") == 0)
        ctx->texts[ctx->texts_count].color = is_dark ? 0xFFFFFFFF : 0xFF121212;
      else
        ctx->texts[ctx->texts_count].color = 0xFFFFFFFF;
      ctx->texts[ctx->texts_count].size = 16;
      ctx->texts_count++;
    }
  } else if (warp_strcmp(node->tag, "input") == 0) {
    node->w = limit_w;
    node->h = 48;
    
    char out_var[128], val[256], placeholder[128];
    eval_attr(ctx, node, "output", out_var, 127);
    eval_attr(ctx, node, "placeholder", placeholder, 127);
    
    val[0] = '\0';
    if (out_var[0]) { warp_strcpy(val, get_state(ctx, out_var)); }
    
    if (ctx->texts_count < MAX_TEXTS) {
      ctx->texts[ctx->texts_count].x = node->x + 12;
      ctx->texts[ctx->texts_count].y = node->y + 16;
      
      if (val[0]) {
        warp_strcpy(ctx->texts[ctx->texts_count].text, val);
        ctx->texts[ctx->texts_count].color = is_dark ? 0xFFCCCCCC : 0xFF333333;
      } else {
        warp_strcpy(ctx->texts[ctx->texts_count].text, placeholder);
        ctx->texts[ctx->texts_count].color = is_dark ? 0xFF666666 : 0xFF888888;
      }
      ctx->texts[ctx->texts_count].size = 16;
      ctx->texts_count++;
    }
  } else if (warp_strcmp(node->tag, "hStack") == 0) {
    int cx = px;
    int max_h = 0;
    int count = node->children_count;
    if (count > 0) {
      int item_w = (limit_w - (count - 1) * 12) / count;
      for (int i = 0; i < count; i++) {
        int ch = layout_node(ctx, node->children[i], cx, py, item_w);
        if (ch > max_h)
          max_h = ch;
        cx += item_w + 12;
      }
    }
    node->h = max_h;
  } else if (warp_strcmp(node->tag, "vStack") == 0) {
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(ctx, node->children[i], px, cy, limit_w) + 8;
    if (id[0] != '\0') {
      for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
        if (warp_strcmp(ctx->dynamic_nodes[i].target_id, id) == 0) {
          for (int j = 0; j < ctx->dynamic_nodes[i].node_count; j++) {
            cy += layout_node(ctx, ctx->dynamic_nodes[i].nodes[j], px, cy, limit_w) + 8;
          }
        }
      }
    }
    node->h = cy - py;
  } else {
    node->h = 20;
  }

  // Check if dirty
  if (node->x != node->prev_x || node->y != node->prev_y ||
      node->w != node->prev_w || node->h != node->prev_h ||
      node->state_hash != node->prev_state_hash) {
    node->is_dirty = 1;
    ctx->engine_dirty = 1;
  } else {
    node->is_dirty = 0;
  }

  return node->h;
}

static const float K_X[] = {1.498f,  3.381f,  7.456f, 12.630f,
                            17.368f, 21.770f, 30.573f};
static const float K_Y[] = {0.800f,  3.600f,  7.370f, 12.544f,
                            16.619f, 18.502f, 20.000f};

static char* append_squircle_marker(char *p, int x, int y, int w, int h, float radius,
                                    const char *fill, const char *extra) {
  p = warp_stpcpy(p, "<!--BARAM_SQUIRCLE x=\"");
  p = append_int(p, x); p = warp_stpcpy(p, "\" y=\"");
  p = append_int(p, y); p = warp_stpcpy(p, "\" w=\"");
  p = append_int(p, w); p = warp_stpcpy(p, "\" h=\"");
  p = append_int(p, h); p = warp_stpcpy(p, "\" radius=\"");
  p = append_fixed3(p, radius); p = warp_stpcpy(p, "\" fill=\"");
  p = warp_stpcpy(p, fill); p = warp_stpcpy(p, "\" ");
  if (extra && extra[0]) p = warp_stpcpy(p, extra);
  p = warp_stpcpy(p, "-->\n");
  return p;
}



char* emit_squircle_shape_to(char *p, int x, int y, int w, int h, float radius,
                             const char *fill, const char *extra) {
  // radius: -1=デフォルト，0=矩形，1-999=ピクセル値，1000+=パーセンテージ
  // パーセンテージは「取りうる最大角丸（短辺の半分）」に対する割合として扱う。
  if (radius != 0.0f) {
    return append_squircle_marker(p, x, y, w, h, radius, fill, extra);
  }
  if (radius == 0.0f) {
    p = warp_stpcpy(p, "<rect x=\"");
    p = append_int(p, x); p = warp_stpcpy(p, "\" y=\"");
    p = append_int(p, y); p = warp_stpcpy(p, "\" width=\"");
    p = append_int(p, w); p = warp_stpcpy(p, "\" height=\"");
    p = append_int(p, h); p = warp_stpcpy(p, "\" fill=\"");
    p = warp_stpcpy(p, fill); p = warp_stpcpy(p, "\" ");
    p = warp_stpcpy(p, extra); p = warp_stpcpy(p, " />\n");
    return p;
  }
  
  float fw = (float)w, fh = (float)h;
  float fx = (float)x, fy = (float)y;
  
  // 最小辺の半分（100% = パスが崩壊しない最大値）
  float min_edge = (fw < fh) ? fw : fh;
  float max_possible_radius = min_edge / 2.0f;
  
  // radius の解決
  float radius_px;
  if (radius == -1.0f) {
    // デフォルト：自動（元のロジックを使用）
    float s = (fh >= 46.0f) ? 1.15f : 1.0f;
    float edge_x = K_X[6] * s;
    float edge_y = K_Y[6] * s;
    if (edge_x > fw / 2.0f) edge_x = fw / 2.0f;
    if (edge_y > fh / 2.0f) edge_y = fh / 2.0f;
    
    p = warp_stpcpy(p, "<path d=\"M ");
    p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh / 2.0f);
    p = warp_stpcpy(p, " L ");
    p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[0] * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw - K_X[1] * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[2] * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[3] * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw - K_X[4] * s); *p++ = ','; p = append_fixed3(p, fy + fh);
    *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[5] * s); *p++ = ','; p = append_fixed3(p, fy + fh);
    *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x); *p++ = ','; p = append_fixed3(p, fy + fh);
    p = warp_stpcpy(p, " L ");
    p = append_fixed3(p, fx + edge_x); *p++ = ','; p = append_fixed3(p, fy + fh);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[5]) * s); *p++ = ','; p = append_fixed3(p, fy + fh);
    *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[4]) * s); *p++ = ','; p = append_fixed3(p, fy + fh);
    *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[3]) * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[2]) * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * s);
    *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[1]) * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * s);
    *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[0]) * s); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * s);
    *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * s);
    *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y);
    p = warp_stpcpy(p, " L ");
    p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[0] * s);
    *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[1] * s);
    *p++ = ' '; p = append_fixed3(p, fx + K_X[0] * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[2] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + K_X[1] * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[3] * s);
    *p++ = ' '; p = append_fixed3(p, fx + K_X[2] * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[4] * s);
    *p++ = ' '; p = append_fixed3(p, fx + K_X[3] * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[5] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + K_X[4] * s); *p++ = ','; p = append_fixed3(p, fy);
    *p++ = ' '; p = append_fixed3(p, fx + K_X[5] * s); *p++ = ','; p = append_fixed3(p, fy);
    *p++ = ' '; p = append_fixed3(p, fx + edge_x); *p++ = ','; p = append_fixed3(p, fy);
    p = warp_stpcpy(p, " L ");
    p = append_fixed3(p, fx + fw - edge_x); *p++ = ','; p = append_fixed3(p, fy);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[5]) * s); *p++ = ','; p = append_fixed3(p, fy);
    *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[4]) * s); *p++ = ','; p = append_fixed3(p, fy);
    *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[3]) * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[5] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[2]) * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[4] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[1]) * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[3] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[0]) * s); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[2] * s);
    p = warp_stpcpy(p, " C ");
    p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[1] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[0] * s);
    *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y);
    p = warp_stpcpy(p, " L ");
    p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh / 2.0f);
    p = warp_stpcpy(p, " Z\" fill=\"");
    p = warp_stpcpy(p, fill); p = warp_stpcpy(p, "\" ");
    p = warp_stpcpy(p, extra); p = warp_stpcpy(p, " />\n");
    return p;
  } else if (radius >= 1000.0f) {
    // パーセンテージ表記
    float radius_pct = radius - 1000.0f;
    if (radius_pct > 100.0f) radius_pct = 100.0f;
    if (radius_pct < 0.0f) radius_pct = 0.0f;
    radius_px = (max_possible_radius * radius_pct) / 100.0f;
  } else {
    // ピクセル値
    radius_px = radius;
  }
  
  // 上限制限：短辺の半分を超えない
  if (radius_px > max_possible_radius) radius_px = max_possible_radius;
  
  // edge_x, edge_y を radius から逆算
  // K_X[6], K_Y[6] はデフォルトの corner size（約 30.573, 20.0）
  // radius_px を元にスケーリング
  float s = (fh >= 46.0f) ? 1.15f : 1.0f;
  float default_radius = 12.0f * s; // デフォルト radius
  float scale = (default_radius > 0.0f) ? (radius_px / default_radius) : 1.0f;
  
  // scale が大きすぎる場合は制限（元の corner が矩形の半分を超えないように）
  float max_scale_x = (fw / 2.0f) / (K_X[6] * s);
  float max_scale_y = (fh / 2.0f) / (K_Y[6] * s);
  float max_scale = (max_scale_x < max_scale_y) ? max_scale_x : max_scale_y;
  if (scale > max_scale) scale = max_scale;
  if (scale < 0.0f) scale = 0.0f;
  
  float curve_scale = s * scale;
  float edge_x = K_X[6] * curve_scale;
  float edge_y = K_Y[6] * curve_scale;
  
  if (edge_x > fw / 2.0f) edge_x = fw / 2.0f;
  if (edge_y > fh / 2.0f) edge_y = fh / 2.0f;
  
  p = warp_stpcpy(p, "<path d=\"M ");
  p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh / 2.0f);
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[0] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - K_X[1] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[2] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[3] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - K_X[4] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh);
  *p++ = ' '; p = append_fixed3(p, fx + fw - K_X[5] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh);
  *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x); *p++ = ','; p = append_fixed3(p, fy + fh);
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + edge_x); *p++ = ','; p = append_fixed3(p, fy + fh);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[5]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh);
  *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[4]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh);
  *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[3]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[2]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[1]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[0]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + fh - edge_y);
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[0] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[1] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + K_X[0] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[2] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + K_X[1] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[3] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + K_X[2] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[4] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + K_X[3] * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[5] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + K_X[4] * curve_scale); *p++ = ','; p = append_fixed3(p, fy);
  *p++ = ' '; p = append_fixed3(p, fx + K_X[5] * curve_scale); *p++ = ','; p = append_fixed3(p, fy);
  *p++ = ' '; p = append_fixed3(p, fx + edge_x); *p++ = ','; p = append_fixed3(p, fy);
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + fw - edge_x); *p++ = ','; p = append_fixed3(p, fy);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[5]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy);
  *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[4]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy);
  *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[3]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[5] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[2]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[4] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[1]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[3] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[0]) * curve_scale); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[2] * curve_scale);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[1] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y - K_Y[0] * curve_scale);
  *p++ = ' '; p = append_fixed3(p, fx + fw); *p++ = ','; p = append_fixed3(p, fy + edge_y);
  p = warp_stpcpy(p, " Z\" fill=\"");
  p = warp_stpcpy(p, fill);
  p = warp_stpcpy(p, "\" ");
  p = warp_stpcpy(p, extra);
  p = warp_stpcpy(p, " />\n");
  p = append_squircle_marker(p, x, y, w, h, radius, fill, extra);
  return p;
}

void warp_context_set_state(warp_context_t *ctx, const char *key, const char *val) {
  set_state(ctx, key, val);
  if (warp_strcasecmp(key, "_currentScreen") == 0) {
    parse_current_screen_classic(ctx);
  }
  ctx->engine_dirty = 1;
}

void warp_context_set_mouse(warp_context_t* ctx, int x, int y) {
  if (ctx->mouse_x != x || ctx->mouse_y != y) {
    ctx->mouse_x = x;
    ctx->mouse_y = y;
    if (warp_strcasecmp(get_state(ctx, "dev pointcheck"), "true") == 0) {
      ctx->engine_dirty = 1;
    }
  }
}

static void emit_svg_recursive(warp_context_t *ctx, warp_node_t *node, char *dest, int dest_size) {
  if (!node) return;
  const char *id = get_attr(node, "id");
  if (!get_visibility(ctx, id)) return;
  
  const char *dark_val = get_state(ctx, "~~main/dark");
  int is_dark = (warp_strcmp(dark_val, "true") == 0);

  if (warp_strcmp(node->tag, "screen") == 0) {
    char bg_color[32], bg_opacity[16];
    eval_attr(ctx, node, "backgroundColor", bg_color, sizeof(bg_color));
    eval_attr(ctx, node, "backgroundOpacity", bg_opacity, sizeof(bg_opacity));

    const char *fill = "transparent";
    char extra[64] = "";
    emit_squircle_shape_to(dest + warp_strlen(dest), 0, 0, node->w, node->h, 0, fill, extra);
  } else if (warp_strcmp(node->tag, "Header") == 0) {
    // Header background and content handled by system title bar
    return;
  } else if (warp_strcmp(node->tag, "card") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : (is_dark ? "#1e1e1e" : "#ffffff");
    char extra[256];
    if (is_dark) {
        warp_strcpy(extra, "stroke=\"#333333\" stroke-width=\"1\"");
    } else {
        warp_strcpy(extra, "stroke=\"#dddddd\" stroke-width=\"1\"");
    }
    emit_squircle_shape_to(dest + warp_strlen(dest), node->x, node->y, node->w, node->h, 12.0f, fill, extra);
  } else if (warp_strcmp(node->tag, "button") == 0 || warp_strcmp(node->tag, "tonalButton") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : "#0a56d0";
    if (warp_strcmp(node->tag, "tonalButton") == 0) {
      const char *tonal_fill = is_dark ? "#ffffff" : "#000000";
      emit_squircle_shape_to(dest + warp_strlen(dest), node->x, node->y, node->w, node->h, -1.0f, tonal_fill, "opacity=\"0.1\"");
    } else {
      emit_squircle_shape_to(dest + warp_strlen(dest), node->x, node->y, node->w, node->h, -1.0f, fill, "");
    }
  } else if (warp_strcmp(node->tag, "switch") == 0) {
    // スイッチの描画
    char out_var[128];
    get_switch_state_key(node, out_var, sizeof(out_var));

    const char *val = get_state(ctx, out_var);
    int on = (warp_strstr(val, "true") != NULL);
    int disabled = (warp_strstr(val, "Disabled") != NULL);

    // 背景
    const char *bg_color = on ? "#0A60FF" : "#dddddd";
    if (disabled)
      bg_color = on ? "#80A0FF" : "#eeeeee";
    int size = 44;
    int x = node->x + (node->w - size) / 2;
    int y = node->y + (node->h - size) / 2;
    emit_squircle_shape_to(dest + warp_strlen(dest), x, y, size, size, 1050.0f,
                           bg_color, disabled ? "opacity=\"0.5\"" : "");

    // チェックマーク（true の場合のみ）
    if (on) {
      char check_buf[512];
      char *p = check_buf;
      p = warp_stpcpy(p, "<path d=\"M");
      p = append_int(p, x + 12); p = warp_stpcpy(p, " ");
      p = append_int(p, y + 22);
      p = warp_stpcpy(p, " L"); p = append_int(p, x + 20); p = warp_stpcpy(p, " "); p = append_int(p, y + 30);
      p = warp_stpcpy(p, " L"); p = append_int(p, x + 34); p = warp_stpcpy(p, " "); p = append_int(p, y + 14);
      p = warp_stpcpy(p, "\" stroke=\"#ffffff\" stroke-width=\"4\" fill=\"none\" />\n");
      warp_strncat(dest, check_buf, dest_size - warp_strlen(dest) - 1);
    }
  } else if (warp_strcmp(node->tag, "slider") == 0) {
    char key[128];
    get_slider_state_key(node, key, sizeof(key));
    const char *val = get_state(ctx, key);
    int v = (int)warp_strtol(val);
    if (v < 0) v = 0;
    if (v > 100) v = 100;

    emit_squircle_shape_to(dest + warp_strlen(dest), node->x, node->y + 14, node->w, 4, 2.0f, "#dddddd", "");

    int knob_x = node->x + (node->w * v / 100) - 10;
    if (knob_x < node->x - 10) knob_x = node->x - 10;
    if (knob_x > node->x + node->w - 10) knob_x = node->x + node->w - 10;
    emit_squircle_shape_to(dest + warp_strlen(dest), knob_x, node->y + 6, 20, 20, 10.0f, "#0A60FF", "");
  } else if (warp_strcmp(node->tag, "input") == 0) {
    // 入力フォームの描画 - 角丸矩形
    const char *stroke = is_dark ? "#555555" : "#dddddd";
    const char *stroke_w = "1";
    
    // フォーカス判定があればここで強調 (Classicエンジンでは現状簡易)
    char extra[128];
    warp_strcpy(extra, "stroke=\""); warp_strcat(extra, stroke);
    warp_strcat(extra, "\" stroke-width=\""); warp_strcat(extra, stroke_w); warp_strcat(extra, "\"");
    
    emit_squircle_shape_to(dest + warp_strlen(dest), node->x, node->y, node->w, node->h, 8.0f, is_dark ? "#333333" : "#ffffff", extra);
  }

  for (int i = 0; i < node->children_count; i++) {
    if (warp_strcmp(node->children[i]->tag, "Header") != 0)
      emit_svg_recursive(ctx, node->children[i], dest, dest_size);
  }
  
  if (id[0] != '\0') {
    for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
      if (warp_strcmp(ctx->dynamic_nodes[i].target_id, id) == 0) {
        for (int j = 0; j < ctx->dynamic_nodes[i].node_count; j++) {
          if (warp_strcmp(ctx->dynamic_nodes[i].nodes[j]->tag, "Header") != 0)
            emit_svg_recursive(ctx, ctx->dynamic_nodes[i].nodes[j], dest, dest_size);
        }
      }
    }
  }

  // Header background and content are now NOT handled here.
  // System title bar handles it.

  // Draw hitboxes if devEventCheck=true
  if (warp_strcmp(get_state(ctx, "devEventCheck"), "true") == 0) {
    int has_hitbox = (node->event_oneclick[0] != '\0' ||
                      node->event_longpress[0] != '\0' ||
                      warp_strcmp(node->tag, "button") == 0 ||
                      warp_strcmp(node->tag, "tonalButton") == 0 ||
                      warp_strcmp(node->tag, "switch") == 0);
    if (has_hitbox && node->w > 0 && node->h > 0) {
      char rect[256];
      char *p = rect;
      p = warp_stpcpy(p, "<rect x=\"");
      p = append_int(p, node->x); p = warp_stpcpy(p, "\" y=\"");
      p = append_int(p, node->y); p = warp_stpcpy(p, "\" width=\"");
      p = append_int(p, node->w); p = warp_stpcpy(p, "\" height=\"");
      p = append_int(p, node->h); p = warp_stpcpy(p, "\" fill=\"red\" opacity=\"0.4\" stroke=\"red\" stroke-width=\"2\" />\n");
      warp_strncat(dest, rect, dest_size - warp_strlen(dest) - 1);
    }
  }
}

static unsigned char hex_nibble(const char c) {
  if (c >= '0' && c <= '9') return (unsigned char)(c - '0');
  if (c >= 'a' && c <= 'f') return (unsigned char)(c - 'a' + 10);
  if (c >= 'A' && c <= 'F') return (unsigned char)(c - 'A' + 10);
  return 0;
}

static void parse_hex_rgba(const char *hex, float opacity,
                           unsigned char *r, unsigned char *g, unsigned char *b, unsigned char *a) {
  if (opacity < 0.0f) opacity = 0.0f;
  if (opacity > 1.0f) opacity = 1.0f;
  *r = *g = *b = 0;
  *a = (unsigned char)(opacity * 255.0f + 0.5f);
  if (!hex || hex[0] != '#') return;
  if (hex[1] && hex[2] && hex[3] && !hex[4]) {
    *r = (unsigned char)((hex_nibble(hex[1]) << 4) | hex_nibble(hex[1]));
    *g = (unsigned char)((hex_nibble(hex[2]) << 4) | hex_nibble(hex[2]));
    *b = (unsigned char)((hex_nibble(hex[3]) << 4) | hex_nibble(hex[3]));
    return;
  }
  if (hex[1] && hex[2] && hex[3] && hex[4] && hex[5] && hex[6]) {
    *r = (unsigned char)((hex_nibble(hex[1]) << 4) | hex_nibble(hex[2]));
    *g = (unsigned char)((hex_nibble(hex[3]) << 4) | hex_nibble(hex[4]));
    *b = (unsigned char)((hex_nibble(hex[5]) << 4) | hex_nibble(hex[6]));
  }
}

static void append_squircle_op(warp_context_t *ctx, int x, int y, int w, int h,
                               float radius, const char *fill, float opacity,
                               const char *stroke, float stroke_width) {
  if (!ctx || w <= 0 || h <= 0) return;
  ensure_draw_ops_capacity(ctx);
  warp_draw_op_t *op = &ctx->draw_ops[ctx->draw_ops_count++];
  *op = (warp_draw_op_t){0};
  op->type = WARP_DRAW_SQUIRCLE;
  op->x = (float)x; op->y = (float)y; op->w = (float)w; op->h = (float)h;
  op->radius = radius;
  if (fill && fill[0] == '#') {
    op->has_fill = 1;
    parse_hex_rgba(fill, opacity, &op->fr, &op->fg, &op->fb, &op->fa);
  }
  if (stroke && stroke[0] == '#' && stroke_width > 0.0f) {
    op->has_stroke = 1;
    op->stroke_width = stroke_width;
    parse_hex_rgba(stroke, opacity, &op->sr, &op->sg, &op->sb, &op->sa);
  }
}

static void append_line_op(warp_context_t *ctx, int x1, int y1, int x2, int y2,
                           const char *stroke, float stroke_width, float opacity) {
  if (!ctx || !stroke || stroke[0] != '#') return;
  ensure_draw_ops_capacity(ctx);
  warp_draw_op_t *op = &ctx->draw_ops[ctx->draw_ops_count++];
  *op = (warp_draw_op_t){0};
  op->type = WARP_DRAW_LINE;
  op->x = (float)x1; op->y = (float)y1; op->x2 = (float)x2; op->y2 = (float)y2;
  op->stroke_width = stroke_width;
  op->has_stroke = 1;
  parse_hex_rgba(stroke, opacity, &op->sr, &op->sg, &op->sb, &op->sa);
}

static void emit_draw_recursive(warp_context_t *ctx, warp_node_t *node) {
  if (!node) return;
  const char *id = get_attr(node, "id");
  if (!get_visibility(ctx, id)) return;

  const char *dark_val = get_state(ctx, "~~main/dark");
  int is_dark = (warp_strcmp(dark_val, "true") == 0);

  if (warp_strcmp(node->tag, "Header") == 0) {
    return;
  } else if (warp_strcmp(node->tag, "card") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : (is_dark ? "#1e1e1e" : "#ffffff");
    append_squircle_op(ctx, node->x, node->y, node->w, node->h, 12.0f, fill, 1.0f,
                       is_dark ? "#333333" : "#dddddd", 1.0f);
  } else if (warp_strcmp(node->tag, "button") == 0 || warp_strcmp(node->tag, "tonalButton") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : "#0a56d0";
    if (warp_strcmp(node->tag, "tonalButton") == 0)
      append_squircle_op(ctx, node->x, node->y, node->w, node->h, -1.0f, is_dark ? "#ffffff" : "#000000", 0.1f, NULL, 0.0f);
    else
      append_squircle_op(ctx, node->x, node->y, node->w, node->h, -1.0f, fill, 1.0f, NULL, 0.0f);
  } else if (warp_strcmp(node->tag, "switch") == 0) {
    char out_var[128];
    get_switch_state_key(node, out_var, sizeof(out_var));
    const char *val = get_state(ctx, out_var);
    int on = (warp_strstr(val, "true") != NULL);
    int disabled = (warp_strstr(val, "Disabled") != NULL);
    const char *bg_color = on ? "#0A60FF" : "#dddddd";
    if (disabled) bg_color = on ? "#80A0FF" : "#eeeeee";
    int size = 44;
    int x = node->x + (node->w - size) / 2;
    int y = node->y + (node->h - size) / 2;
    append_squircle_op(ctx, x, y, size, size, 1050.0f, bg_color, disabled ? 0.5f : 1.0f, NULL, 0.0f);
    if (on) {
      append_line_op(ctx, x + 12, y + 22, x + 20, y + 30, "#ffffff", 4.0f, 1.0f);
      append_line_op(ctx, x + 20, y + 30, x + 34, y + 14, "#ffffff", 4.0f, 1.0f);
    }
  } else if (warp_strcmp(node->tag, "slider") == 0) {
    char key[128];
    get_slider_state_key(node, key, sizeof(key));
    const char *val = get_state(ctx, key);
    int v = (int)warp_strtol(val);
    if (v < 0) v = 0;
    if (v > 100) v = 100;
    append_squircle_op(ctx, node->x, node->y + 14, node->w, 4, 2.0f, "#dddddd", 1.0f, NULL, 0.0f);
    int knob_x = node->x + (node->w * v / 100) - 10;
    if (knob_x < node->x - 10) knob_x = node->x - 10;
    if (knob_x > node->x + node->w - 10) knob_x = node->x + node->w - 10;
    append_squircle_op(ctx, knob_x, node->y + 6, 20, 20, 10.0f, "#0A60FF", 1.0f, NULL, 0.0f);
  } else if (warp_strcmp(node->tag, "input") == 0) {
    append_squircle_op(ctx, node->x, node->y, node->w, node->h, 8.0f,
                       is_dark ? "#333333" : "#ffffff", 1.0f,
                       is_dark ? "#555555" : "#dddddd", 1.0f);
  }

  for (int i = 0; i < node->children_count; i++) {
    if (warp_strcmp(node->children[i]->tag, "Header") != 0)
      emit_draw_recursive(ctx, node->children[i]);
  }

  if (id[0] != '\0') {
    for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
      if (warp_strcmp(ctx->dynamic_nodes[i].target_id, id) == 0) {
        for (int j = 0; j < ctx->dynamic_nodes[i].node_count; j++) {
          if (warp_strcmp(ctx->dynamic_nodes[i].nodes[j]->tag, "Header") != 0)
            emit_draw_recursive(ctx, ctx->dynamic_nodes[i].nodes[j]);
        }
      }
    }
  }

  if (warp_strcmp(get_state(ctx, "devEventCheck"), "true") == 0) {
    int has_hitbox = (node->event_oneclick[0] != '\0' ||
                      node->event_longpress[0] != '\0' ||
                      warp_strcmp(node->tag, "button") == 0 ||
                      warp_strcmp(node->tag, "tonalButton") == 0 ||
                      warp_strcmp(node->tag, "switch") == 0);
    if (has_hitbox && node->w > 0 && node->h > 0)
      append_squircle_op(ctx, node->x, node->y, node->w, node->h, 0.0f, "#ff0000", 0.4f, "#ff0000", 2.0f);
  }
}

static void emit_svg(warp_context_t *ctx, warp_node_t *node) {
  emit_svg_recursive(ctx, node, ctx->svg_output, sizeof(ctx->svg_output));

  // Draw pointcheck 3x3 green box at mouse position
  if (warp_strcasecmp(get_state(ctx, "dev pointcheck"), "true") == 0) {
    char rect[128];
    char *p = rect;
    p = warp_stpcpy(p, "<rect x=\"");
    p = append_int(p, ctx->mouse_x - 1); p = warp_stpcpy(p, "\" y=\"");
    p = append_int(p, ctx->mouse_y - 1); p = warp_stpcpy(p, "\" width=\"3\" height=\"3\" fill=\"#00FF00\" />\n");
    warp_strncat(ctx->svg_output, rect, sizeof(ctx->svg_output) - warp_strlen(ctx->svg_output) - 1);
  }
}

static void update_status_info(warp_context_t *ctx) {
  char buf[128];
  buf[0] = '\0';
  warp_strcat(buf, "Nodes:");
  char n_str[16];
  append_int(n_str, ctx->nodes_count);
  warp_strcat(buf, n_str);
  warp_strcat(buf, " Tokens:");
  append_int(n_str, ctx->token_count);
  warp_strcat(buf, n_str);
  warp_strcat(buf, " State:");
  if (ctx->root_nodes_count > 0)
    warp_strcat(buf, "OK");
  else
    warp_strcat(buf, "NoRoot");
  warp_strncpy(ctx->engine_status, buf, 127);
}

static void execute_action(warp_context_t *ctx, const char *action);

static long eval_calc_expr(const char *s) {
  const char *p = s;
  while (*p == ' ' || *p == '\t')
    p++;
  if (!*p)
    return 0;
  long res = warp_strtol(p);
  while (*p && (*p == ' ' || *p == '\t' || *p == '-' || (*p >= '0' && *p <= '9')))
    p++;
  while (*p) {
    while (*p == ' ' || *p == '\t')
      p++;
    if (!*p)
      break;
    char op = *p++;
    long v = warp_strtol(p);
    if (op == '+')
      res += v;
    else if (op == '-')
      res -= v;
    else if (op == '*')
      res *= v;
    else if (op == '/' && v != 0)
      res /= v;
    while (*p && (*p == ' ' || *p == '\t' || *p == '-' || (*p >= '0' && *p <= '9')))
      p++;
  }
  return res;
}

static char *evaluate_rhs(warp_context_t *ctx, const char *expr, char *out, int max_len) {
  const char *replace = warp_strstr(expr, ".replace{");
  if (replace) {
    char base_expr[256], old_expr[128], new_expr[128];
    char base[512], old_val[128], new_val[128];
    int base_len = replace - expr;
    if (base_len >= (int)sizeof(base_expr))
      base_len = (int)sizeof(base_expr) - 1;
    warp_strncpy(base_expr, expr, base_len);
    base_expr[base_len] = '\0';

    const char *args = replace + 9;
    const char *close = warp_strchr(args, '}');
    const char *comma = warp_strchr(args, ',');
    if (!close || !comma || comma > close) {
      eval_expr(ctx, expr, out, max_len);
      return out;
    }

    int old_len = comma - args;
    if (old_len >= (int)sizeof(old_expr))
      old_len = (int)sizeof(old_expr) - 1;
    warp_strncpy(old_expr, args, old_len);
    old_expr[old_len] = '\0';

    int new_len = close - (comma + 1);
    if (new_len >= (int)sizeof(new_expr))
      new_len = (int)sizeof(new_expr) - 1;
    warp_strncpy(new_expr, comma + 1, new_len);
    new_expr[new_len] = '\0';

    eval_expr(ctx, base_expr, base, sizeof(base));
    eval_expr(ctx, old_expr, old_val, sizeof(old_val));
    eval_expr(ctx, new_expr, new_val, sizeof(new_val));

    char *found = old_val[0] ? warp_strstr(base, old_val) : NULL;
    if (!found) {
      warp_strncpy(out, base, max_len - 1);
      out[max_len - 1] = '\0';
      return out;
    }

    int head = found - base;
    int old_val_len = warp_strlen(old_val);
    out[0] = '\0';
    if (head > 0)
      warp_strncat(out, base, head);
    warp_strncat(out, new_val, max_len - warp_strlen(out) - 1);
    warp_strncat(out, found + old_val_len, max_len - warp_strlen(out) - 1);
    return out;
  }

  if (warp_strncmp(expr, "calc(", 5) == 0) {
    const char *p = expr + 5;
    char sub_expr[512] = "";
    while (*p && *p != ')') {
      if (warp_strncmp(p, "--", 2) == 0) {
        char var[64];
        int i = 0;
        while (*p && *p != '\"' && *p != '+' && *p != '-' && *p != '*' &&
               *p != '/' && *p != ' ' && *p != ')' && i < 63)
          var[i++] = *p++;
        var[i] = '\0';
        warp_strncat(sub_expr, get_state(ctx, var), 511 - warp_strlen(sub_expr));
      } else {
        int len = warp_strlen(sub_expr);
        if (len < 511) {
          sub_expr[len] = *p;
          sub_expr[len + 1] = '\0';
        }
        p++;
      }
    }
    long res = eval_calc_expr(sub_expr);
    append_int(out, (int)res);
    return out;
  }
  eval_expr(ctx, expr, out, max_len);
  return out;
}

static int evaluate_condition(warp_context_t *ctx, const char *cond) {
  char left_expr[256], right_expr[256], left_val[512], right_val[512];
  const char *eq = warp_strchr(cond, '=');
  if (!eq)
    return 0;
  int len = eq - cond;
  if (len >= 256)
    len = 255;
  warp_strncpy(left_expr, cond, len);
  left_expr[len] = '\0';
  warp_strcpy(right_expr, eq + 1);
  eval_expr(ctx, left_expr, left_val, 511);
  eval_expr(ctx, right_expr, right_val, 511);
  return warp_strcmp(left_val, right_val) == 0;
}

static void execute_script(warp_context_t *ctx, const char *name) {
  for (int i = 0; i < ctx->scripts_count; i++) {
    if (warp_strcmp(ctx->scripts[i].name, name) == 0) {
      int matched_if = 0;
      for (int j = 0; j < ctx->scripts[i].block_count; j++) {
        script_block_t *b = &ctx->scripts[i].blocks[j];
        if (warp_strcmp(b->type, "if") == 0) {
          if (evaluate_condition(ctx, b->condition)) {
            execute_action(ctx, b->actions);
            matched_if = 1;
          }
        } else if (warp_strcmp(b->type, "elseIf") == 0) {
          if (!matched_if && evaluate_condition(ctx, b->condition)) {
            execute_action(ctx, b->actions);
            matched_if = 1;
          }
        }
      }
      return;
    }
  }
}

extern void sys_restart(void);
static void execute_action(warp_context_t *ctx, const char *action_str) {
  if (!action_str || !action_str[0])
    return;
  char buf[1024];
  warp_strncpy(buf, action_str, 1023);
  char *p = buf;
  while (*p) {
    char *start = p;
    int paren = 0, in_quote = 0;
    while (*p) {
      if (*p == '\"')
        in_quote = !in_quote;
      if (!in_quote) {
        if (*p == '(')
          paren++;
        else if (*p == ')')
          paren--;
        if (*p == ',' && paren == 0)
          break;
      }
      p++;
    }
    char action[512];
    int len = p - start;
    if (len >= 512)
      len = 511;
    warp_strncpy(action, start, len);
    action[len] = '\0';
    if (*p == ',')
      p++;
    char *act = action;
    while (*act == ' ' || *act == '\t')
      act++;
    if (warp_strcmp(act, "restart(now)") == 0 || warp_strcmp(act, "reset(now)") == 0) {
      sys_restart();
    } else if (warp_strncmp(act, "setScreen(", 10) == 0) {
      char screen[64];
      warp_strncpy(screen, act + 10, 63);
      char *end = warp_strchr(screen, ')');
      if (end)
        *end = '\0';
      // 画面切り替え - スクロール状態は画面ごとに保持される
      warp_strncpy(ctx->current_screen, screen, 63);
    } else if (warp_strncmp(act, "show(", 5) == 0) {
      char id[64];
      warp_strncpy(id, act + 5, 63);
      char *end = warp_strchr(id, ')');
      if (end)
        *end = '\0';
      set_visibility(ctx, id, 1);
    } else if (warp_strncmp(act, "hide(", 5) == 0) {
      char id[64];
      warp_strncpy(id, act + 5, 63);
      char *end = warp_strchr(id, ')');
      if (end)
        *end = '\0';
      set_visibility(ctx, id, 0);
    } else if (warp_strncmp(act, "script(", 7) == 0) {
      char name[64];
      warp_strncpy(name, act + 7, 63);
      char *end = warp_strchr(name, ')');
      if (end)
        *end = '\0';
      execute_script(ctx, name);
    } else if (warp_strncmp(act, "add(", 4) == 0) {
      char inner[512];
      warp_strncpy(inner, act + 4, 511);
      char *end = warp_strchr(inner, ')');
      if (end)
        *end = '\0';
      char *colon = warp_strchr(inner, ':');
      if (colon) {
        *colon = '\0';
        char target_id[64];
        warp_strncpy(target_id, inner, 63);
        char *code = colon + 1;
        while (*code == ' ' || *code == '\t' || *code == '\'' || *code == '\"')
          code++;
        char *code_end = code + warp_strlen(code) - 1;
        while (code_end > code &&
               (*code_end == ' ' || *code_end == '\t' || *code_end == '\'' ||
                *code_end == '\"')) {
          *code_end = '\0';
          code_end--;
        }
        const char *old_src = ctx->src_ptr;
        int prev_token_count = ctx->token_count, prev_token_pos = ctx->token_pos;
        ctx->src_ptr = code;
        int start_pos = ctx->token_count;
        while (1) {
          token_t tk = next_token(ctx);
          if (tk.type == TK_EOF || ctx->token_count >= MAX_TOKENS)
            break;
          ctx->tokens[ctx->token_count++] = tk;
        }
        ctx->token_pos = start_pos;
        warp_node_t *new_node = parse_node(ctx);
        ctx->src_ptr = old_src;
        ctx->token_count = prev_token_count;
        ctx->token_pos = prev_token_pos;
        if (new_node) {
          new_node->is_dynamic = 1;
          int found = 0;
          for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
            if (warp_strcmp(ctx->dynamic_nodes[i].target_id, target_id) == 0) {
              if (ctx->dynamic_nodes[i].node_count < 16)
                ctx->dynamic_nodes[i].nodes[ctx->dynamic_nodes[i].node_count++] =
                    new_node;
              found = 1;
              break;
            }
          }
          if (!found && ctx->dynamic_nodes_count < MAX_DYNAMIC_NODES) {
            warp_strncpy(ctx->dynamic_nodes[ctx->dynamic_nodes_count].target_id,
                         target_id, 63);
            ctx->dynamic_nodes[ctx->dynamic_nodes_count].nodes[0] = new_node;
            ctx->dynamic_nodes[ctx->dynamic_nodes_count].node_count = 1;
            ctx->dynamic_nodes_count++;
          }
        }
      }
    } else if (warp_strncmp(act, "del(", 4) == 0) {
      char inner[512];
      warp_strncpy(inner, act + 4, 511);
      char *end = warp_strchr(inner, ')');
      if (end)
        *end = '\0';
      char *colon = warp_strchr(inner, ':');
      char *target_id = inner, *tag = NULL;
      if (colon) {
        *colon = '\0';
        tag = colon + 1;
        while (*tag == ' ' || *tag == '\t')
          tag++;
      }
      for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
        if (warp_strcmp(ctx->dynamic_nodes[i].target_id, target_id) == 0) {
          if (tag) {
            for (int j = ctx->dynamic_nodes[i].node_count - 1; j >= 0; j--) {
              if (warp_strcmp(ctx->dynamic_nodes[i].nodes[j]->tag, tag) == 0) {
                for (int k = j; k < ctx->dynamic_nodes[i].node_count - 1; k++)
                  ctx->dynamic_nodes[i].nodes[k] = ctx->dynamic_nodes[i].nodes[k + 1];
                ctx->dynamic_nodes[i].node_count--;
                break;
              }
            }
          } else if (ctx->dynamic_nodes[i].node_count > 0)
            ctx->dynamic_nodes[i].node_count--;
          break;
        }
      }
    } else if (warp_strncmp(act, "clr(", 4) == 0) {
      char target_id[64];
      warp_strncpy(target_id, act + 4, 63);
      char *end = warp_strchr(target_id, ')');
      if (end)
        *end = '\0';
      for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
        if (warp_strcmp(ctx->dynamic_nodes[i].target_id, target_id) == 0) {
          ctx->dynamic_nodes[i].node_count = 0;
          break;
        }
      }
    } else if (warp_strncmp(act, "--", 2) == 0) {
      char *eq = warp_strchr(act, '=');
      if (!eq)
        eq = warp_strchr(act, ':');
      if (eq) {
        char key[64], val_expr[512], val[512];
        int klen = eq - act;
        if (klen >= 64)
          klen = 63;
        warp_strncpy(key, act, klen);
        key[klen] = '\0';
        warp_strcpy(val_expr, eq + 1);
        evaluate_rhs(ctx, val_expr, val, sizeof(val));
        set_state(ctx, key, val);
      }
    }
  }
}

static int check_clicks(warp_context_t *ctx, warp_node_t *node, int x, int y) {
  if (!node)
    return 0;
  const char *id = get_attr(node, "id");
  if (!get_visibility(ctx, id))
    return 0;
  // 子ノードからチェック（逆順）
  for (int i = node->children_count - 1; i >= 0; i--) {
    if (check_clicks(ctx, node->children[i], x, y))
      return 1;
  }
  // dynamic nodes
  if (id[0] != '\0') {
    for (int i = 0; i < ctx->dynamic_nodes_count; i++) {
      if (warp_strcmp(ctx->dynamic_nodes[i].target_id, id) == 0) {
        for (int j = ctx->dynamic_nodes[i].node_count - 1; j >= 0; j--) {
          if (check_clicks(ctx, ctx->dynamic_nodes[i].nodes[j], x, y))
            return 1;
        }
      }
    }
  }
  // このノードのヒット判定
  if (x >= node->x && x <= node->x + node->w && y >= node->y &&
      y <= node->y + node->h) {
    if (warp_strcmp(node->tag, "switch") == 0) {
      char out_var[128];
      get_switch_state_key(node, out_var, sizeof(out_var));

      if (out_var[0]) {
        const char *current = get_state(ctx, out_var);
        // "Disabled"が含まれる場合はクリックさせない
        if (warp_strstr(current, "Disabled") == NULL) {
          int on = (warp_strstr(current, "true") != NULL);
          set_state(ctx, out_var, on ? "false" : "true");
          if (node->event_oneclick[0] != '\0') {
            execute_action(ctx, node->event_oneclick);
          }
        }
      }
      return 1;
    }
    if (warp_strcmp(node->tag, "slider") == 0) {
      for (int i = 0; i < ctx->nodes_count; i++) {
        if (&ctx->nodes[i] == node) {
          ctx->active_slider_idx = i;
          break;
        }
      }
      update_slider_value(ctx, node, x);
      if (node->event_oneclick[0] != '\0') {
        execute_action(ctx, node->event_oneclick);
      }
      return 1;
    }
    if (warp_strcmp(node->tag, "input") == 0) {
      return 1;
    }
    if (warp_strcmp(node->tag, "card") == 0) {
      return 1;
    }
    if (warp_strcmp(node->tag, "button") == 0 || warp_strcmp(node->tag, "tonalButton") == 0) {
      if (node->event_oneclick[0] != '\0') {
        execute_action(ctx, node->event_oneclick);
      }
      return 1;
    }
    if (warp_strcmp(node->tag, "text") == 0) {
      return 1;
    }
    if (node->event_oneclick[0] != '\0') {
      execute_action(ctx, node->event_oneclick);
      return 1;
    }
  }
  return 0;
}

static void skip_block_classic(warp_context_t *ctx) {
  if (ctx->token_pos + 1 >= ctx->token_count || ctx->tokens[ctx->token_pos + 1].val[0] != '(') {
    ctx->token_pos++;
    return;
  }
  ctx->token_pos += 2;
  int depth = 1;
  while (ctx->token_pos < ctx->token_count && depth > 0) {
    if (ctx->tokens[ctx->token_pos].type == TK_PUNCT) {
      if (ctx->tokens[ctx->token_pos].val[0] == '(') depth++;
      else if (ctx->tokens[ctx->token_pos].val[0] == ')') depth--;
    }
    ctx->token_pos++;
  }
}

static void parse_current_screen_classic(warp_context_t *ctx) {
  if (warp_strcmp(ctx->current_screen, ctx->parsed_screen_id) == 0 && ctx->root_nodes_count > 0) return;

  // Clear current UI tree
  ctx->nodes_count = 0;
  ctx->root_nodes_count = 0;
  ctx->texts_count = 0;
  ctx->dynamic_nodes_count = 0;

  for (int i = 0; i < ctx->screens_count_scanned; i++) {
    if (warp_strcmp(ctx->screens[i].id, ctx->current_screen) == 0) {
      ctx->token_pos = ctx->screens[i].token_index;
      warp_node_t *node = parse_node(ctx);
      if (node && ctx->root_nodes_count < 16) {
        ctx->root_nodes[ctx->root_nodes_count++] = node;
        init_state_from_ast(ctx, node);
      }
      warp_strcpy(ctx->parsed_screen_id, ctx->current_screen);
      return;
    }
  }
  ctx->parsed_screen_id[0] = '\0';
}

warp_context_t* warp_context_create(const char* code) {
  warp_context_t* ctx = (warp_context_t*)malloc(sizeof(warp_context_t));
  if (!ctx) return NULL;
  warp_memset(ctx, 0, sizeof(warp_context_t));

  // Allocate dynamic arrays with minimal initial capacities
  ctx->state_capacity = 4;
  ctx->state = (void*)malloc(ctx->state_capacity * sizeof(*ctx->state));
  if (!ctx->state) { free(ctx); return NULL; }

  ctx->screens_capacity = 4;
  ctx->screens = (screen_info_t*)malloc(ctx->screens_capacity * sizeof(screen_info_t));
  if (!ctx->screens) { free(ctx->state); free(ctx); return NULL; }

  ctx->visibility_capacity = 4;
  ctx->visibility = (visibility_t*)malloc(ctx->visibility_capacity * sizeof(visibility_t));
  if (!ctx->visibility) { free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->nodes_capacity = 4;
  ctx->nodes = (warp_node_t*)malloc(ctx->nodes_capacity * sizeof(warp_node_t));
  if (!ctx->nodes) { free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->root_nodes_capacity = 4;
  ctx->root_nodes = (warp_node_t**)malloc(ctx->root_nodes_capacity * sizeof(warp_node_t*));
  if (!ctx->root_nodes) { free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->scripts_capacity = 4;
  ctx->scripts = (script_t*)malloc(ctx->scripts_capacity * sizeof(script_t));
  if (!ctx->scripts) { free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->dynamic_nodes_capacity = 4;
  ctx->dynamic_nodes = (dynamic_nodes_t*)malloc(ctx->dynamic_nodes_capacity * sizeof(dynamic_nodes_t));
  if (!ctx->dynamic_nodes) { free(ctx->scripts); free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->token_capacity = 32;
  ctx->tokens = (token_t*)malloc(ctx->token_capacity * sizeof(token_t));
  if (!ctx->tokens) { free(ctx->dynamic_nodes); free(ctx->scripts); free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->texts_capacity = 4;
  ctx->texts = (void*)malloc(ctx->texts_capacity * sizeof(*ctx->texts));
  if (!ctx->texts) { free(ctx->tokens); free(ctx->dynamic_nodes); free(ctx->scripts); free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->svg_capacity = 512;
  ctx->svg_output = (char*)malloc(ctx->svg_capacity);
  if (!ctx->svg_output) { free(ctx->texts); free(ctx->tokens); free(ctx->dynamic_nodes); free(ctx->scripts); free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  ctx->draw_ops_capacity = 16;
  ctx->draw_ops = (warp_draw_op_t*)malloc(ctx->draw_ops_capacity * sizeof(warp_draw_op_t));
  if (!ctx->draw_ops) { free(ctx->svg_output); free(ctx->texts); free(ctx->tokens); free(ctx->dynamic_nodes); free(ctx->scripts); free(ctx->root_nodes); free(ctx->nodes); free(ctx->visibility); free(ctx->screens); free(ctx->state); free(ctx); return NULL; }

  warp_strcpy(ctx->current_screen, "main");
  warp_strcpy(ctx->engine_status, "Idle");
  ctx->screen_count = 0;
  ctx->screens_count_scanned = 0;
  ctx->parsed_screen_id[0] = '\0';

  if (!code || !code[0]) {
    warp_strncpy(ctx->engine_status, "Err: No Code", 127);
    return ctx;
  }
  
  ctx->src_ptr = code;
  while (1) {
    token_t tk = next_token(ctx);
    if (tk.type == TK_EOF)
      break;
    ensure_tokens_capacity(ctx);
    ctx->tokens[ctx->token_count++] = tk;
  }
  
  ctx->token_pos = 0;
  while (ctx->token_pos < ctx->token_count) {
    if (ctx->tokens[ctx->token_pos].type == TK_AT) {
      parse_script(ctx);
    } else if (ctx->tokens[ctx->token_pos].type == TK_WORD && warp_strcmp(ctx->tokens[ctx->token_pos].val, "screen") == 0) {
      char screen_id[64] = "main";
      int start_pos = ctx->token_pos;
      if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == '(') {
        int j = ctx->token_pos + 2;
        int depth = 1;
        while (j < ctx->token_count && depth > 0) {
          if (ctx->tokens[j].type == TK_PUNCT) {
            if (ctx->tokens[j].val[0] == '(') depth++;
            else if (ctx->tokens[j].val[0] == ')') depth--;
          }
          if (depth == 1 && ctx->tokens[j].type == TK_WORD && 
                   warp_strcmp(ctx->tokens[j].val, "id") == 0 &&
                   j + 1 < ctx->token_count && ctx->tokens[j+1].val[0] == ':') {
            int k = j + 2;
            if (k < ctx->token_count && ctx->tokens[k].val[0] == '(') k++;
            if (k < ctx->token_count && ctx->tokens[k].type != TK_PUNCT) {
              warp_strncpy(screen_id, ctx->tokens[k].val, 63);
            }
          }
          j++;
        }
        if (ctx->screens_count_scanned < MAX_SCREENS) {
          warp_strncpy(ctx->screens[ctx->screens_count_scanned].id, screen_id[0] ? screen_id : "main", 63);
          ctx->screens[ctx->screens_count_scanned].token_index = start_pos;
          ctx->screens_count_scanned++;
        }
        ctx->token_pos = j;
      } else {
        ctx->token_pos++;
      }
    } else {
      skip_block_classic(ctx);
    }
  }

  if (ctx->screens_count_scanned > 0) {
    warp_strcpy(ctx->current_screen, ctx->screens[0].id);
    parse_current_screen_classic(ctx);
  } else {
    warp_strcpy(ctx->current_screen, "main");
  }
  ctx->active_slider_idx = -1;

  update_status_info(ctx);
  ctx->engine_dirty = 1;
  return ctx;
}

void warp_context_destroy(warp_context_t* ctx) {
  if (ctx) {
    free(ctx->state);
    free(ctx->screens);
    free(ctx->visibility);
    free(ctx->nodes);
    free(ctx->root_nodes);
    free(ctx->scripts);
    free(ctx->dynamic_nodes);
    free(ctx->tokens);
    free(ctx->texts);
    free(ctx->svg_output);
    free(ctx->draw_ops);
    free(ctx);
  }
}

void warp_context_update(warp_context_t* ctx, int width, int height) {
  parse_current_screen_classic(ctx);
  ctx->texts_count = 0;
  ctx->svg_output[0] = '\0';
  ctx->draw_ops_count = 0;
  ctx->engine_dirty = 0;
  ctx->win_w = width;
  ctx->win_h = height;

  // Build SVG for current screen only
  int total_h = height;
  for (int i = 0; i < ctx->root_nodes_count; i++) {
    warp_node_t *node = ctx->root_nodes[i];
    int h = layout_node(ctx, node, 0, 0, width);
    if (h > total_h)
      total_h = h;
  }

  for (int i = 0; i < ctx->root_nodes_count; i++) {
    warp_node_t *node = ctx->root_nodes[i];
    if (warp_strcmp(node->tag, "screen") == 0) {
      const char *id = get_attr(node, "id");
      if (warp_strcmp(id, ctx->current_screen) != 0)
        continue;
    }
    emit_draw_recursive(ctx, node);
  }
  if (warp_strcasecmp(get_state(ctx, "dev pointcheck"), "true") == 0)
    append_squircle_op(ctx, ctx->mouse_x - 1, ctx->mouse_y - 1, 3, 3, 0.0f, "#00FF00", 1.0f, NULL, 0.0f);

  // Register/update current screen in screen list (height and scroll only, no SVG string)
  int screen_idx = -1;
  for (int i = 0; i < ctx->screen_count; i++) {
    if (warp_strcmp(ctx->screen_ids[i], ctx->current_screen) == 0) {
      screen_idx = i;
      break;
    }
  }
  if (screen_idx < 0 && ctx->screen_count < MAX_SCREENS) {
    screen_idx = ctx->screen_count++;
    warp_strncpy(ctx->screen_ids[screen_idx], ctx->current_screen, 63);
    ctx->screen_scroll_ys[screen_idx] = 0.0f;
  }
  if (screen_idx >= 0) {
    ctx->screen_content_heights[screen_idx] = total_h;
  }

  update_status_info(ctx);
}

const char* warp_context_get_svg(warp_context_t* ctx) {
  return ctx->svg_output;
}

const warp_draw_op_t* warp_context_get_draw_ops(warp_context_t* ctx, int* out_count) {
  if (!ctx) {
    if (out_count) *out_count = 0;
    return NULL;
  }
  if (out_count) *out_count = ctx->draw_ops_count;
  return ctx->draw_ops;
}

extern void layer_draw_ttf(layer_t *layer, int x, int y, const char *str,
                           float font_size, uint32_t color);

void warp_context_draw_texts(warp_context_t* ctx, layer_t* layer, int off_x, int off_y, float scale) {
  if (!layer)
    return;
  for (int i = 0; i < ctx->texts_count; i++) {
    layer_draw_ttf(layer, (int)((float)ctx->texts[i].x * scale) + off_x, (int)((float)ctx->texts[i].y * scale) + off_y, ctx->texts[i].text, ctx->texts[i].size * scale, ctx->texts[i].color);
  }
}

void warp_context_click(warp_context_t* ctx, int x, int y) {
  parse_current_screen_classic(ctx);
  ctx->active_slider_idx = -1;
  int clicked = 0;
  for (int i = 0; i < ctx->root_nodes_count; i++) {
    warp_node_t *node = ctx->root_nodes[i];
    if (check_clicks(ctx, node, x, y)) {
      clicked = 1;
      break;
    }
  }
  if (!clicked) {
  }
  ctx->engine_dirty = 1;
}

int warp_context_drag_active_slider(warp_context_t* ctx, int x, int y) {
  (void)y;
  if (!ctx || ctx->active_slider_idx < 0 || ctx->active_slider_idx >= ctx->nodes_count) return 0;
  warp_node_t *n = &ctx->nodes[ctx->active_slider_idx];
  if (warp_strcmp(n->tag, "slider") != 0) return 0;
  return update_slider_value(ctx, n, x);
}

void warp_context_end_slider_drag(warp_context_t* ctx) {
  if (!ctx) return;
  ctx->active_slider_idx = -1;
}

int warp_context_get_active_slider_rect(warp_context_t* ctx, int* x, int* y, int* w, int* h) {
  if (!ctx || ctx->active_slider_idx < 0 || ctx->active_slider_idx >= ctx->nodes_count) return 0;
  warp_node_t *n = &ctx->nodes[ctx->active_slider_idx];
  if (warp_strcmp(n->tag, "slider") != 0) return 0;
  if (x) *x = n->x;
  if (y) *y = n->y;
  if (w) *w = n->w;
  if (h) *h = n->h;
  return 1;
}

int warp_context_is_dirty(warp_context_t* ctx) {
  return ctx->engine_dirty;
}

void warp_context_clear_dirty(warp_context_t* ctx) {
  ctx->engine_dirty = 0;
  for (int i = 0; i < ctx->nodes_count; i++) {
    ctx->nodes[i].is_dirty = 0;
  }
}

int warp_context_get_node_count(warp_context_t* ctx) {
  return ctx->nodes_count;
}

void warp_context_get_node_info(warp_context_t* ctx, int index, int* x, int* y, int* w, int* h, int* is_dirty) {
  if (index < 0 || index >= ctx->nodes_count) return;
  *x = ctx->nodes[index].x;
  *y = ctx->nodes[index].y;
  *w = ctx->nodes[index].w;
  *h = ctx->nodes[index].h;
  *is_dirty = ctx->nodes[index].is_dirty;
}

void warp_context_get_node_prev_rect(warp_context_t* ctx, int index, int* x, int* y, int* w, int* h) {
  if (index < 0 || index >= ctx->nodes_count) return;
  *x = ctx->nodes[index].prev_x;
  *y = ctx->nodes[index].prev_y;
  *w = ctx->nodes[index].prev_w;
  *h = ctx->nodes[index].prev_h;
}

const char* warp_context_get_node_prev_svg(warp_context_t* ctx, int index) {
  (void)ctx;
  (void)index;
  return "";
}

const char* warp_context_get_node_svg(warp_context_t* ctx, int index) {
  (void)ctx;
  (void)index;
  return "";
}

const char* warp_context_get_status(warp_context_t* ctx) {
  return ctx->engine_status;
}

static warp_node_t* find_header_node(warp_context_t* ctx) {
  for (int i = 0; i < ctx->nodes_count; i++) {
    warp_node_t* n = &ctx->nodes[i];
    if (warp_strcmp(n->tag, "Header") == 0) {
      return n;
    }
  }
  return NULL;
}

int warp_context_get_header_info(warp_context_t* ctx, char* out_text, int max_len, int* out_action_count) {
  warp_node_t* h = find_header_node(ctx);
  if (!h) return 0;
  
  eval_attr(ctx, h, "text", out_text, max_len);
  *out_action_count = h->children_count;
  return 1;
}

void warp_context_get_header_action_info(warp_context_t* ctx, int action_index, char* out_text, int max_len) {
  warp_node_t* h = find_header_node(ctx);
  if (!h || action_index < 0 || action_index >= h->children_count) {
    out_text[0] = '\0';
    return;
  }
  warp_node_t* action = h->children[action_index];
  eval_attr(ctx, action, "text", out_text, max_len);
}

void warp_context_click_header_action(warp_context_t* ctx, int action_index) {
  warp_node_t* h = find_header_node(ctx);
  if (!h || action_index < 0 || action_index >= h->children_count) return;
  warp_node_t* action = h->children[action_index];
  if (action->event_oneclick[0] != '\0') {
    execute_action(ctx, action->event_oneclick);
    ctx->engine_dirty = 1;
  }
}

int warp_context_is_dev_event_check(warp_context_t* ctx) {
  return warp_strcmp(get_state(ctx, "devEventCheck"), "true") == 0;
}

// Screen-based scroll management
const char* warp_context_get_screen_svg(warp_context_t* ctx, const char* screen_id, int* content_height) {
  if (!ctx || !screen_id) return NULL;
  if (warp_strcmp(ctx->current_screen, screen_id) == 0) {
    if (content_height) {
      *content_height = 0;
      for (int i = 0; i < ctx->screen_count; i++) {
        if (warp_strcmp(ctx->screen_ids[i], screen_id) == 0) {
          *content_height = ctx->screen_content_heights[i];
          break;
        }
      }
    }
    return ctx->svg_output;
  }
  return NULL;
}

void warp_context_set_screen_scroll(warp_context_t* ctx, const char* screen_id, float scroll_y) {
  if (!ctx || !screen_id) return;
  for (int i = 0; i < ctx->screen_count; i++) {
    if (warp_strcmp(ctx->screen_ids[i], screen_id) == 0) {
      ctx->screen_scroll_ys[i] = scroll_y;
      return;
    }
  }
}

float warp_context_get_screen_scroll(warp_context_t* ctx, const char* screen_id) {
  if (!ctx || !screen_id) return 0.0f;
  for (int i = 0; i < ctx->screen_count; i++) {
    if (warp_strcmp(ctx->screen_ids[i], screen_id) == 0) {
      return ctx->screen_scroll_ys[i];
    }
  }
  return 0.0f;
}

// Legacy scroll API (for backward compatibility)
float warp_context_get_scroll_y(warp_context_t* ctx) {
  if (!ctx) return 0.0f;
  return warp_context_get_screen_scroll(ctx, ctx->current_screen);
}

void warp_context_set_scroll_y(warp_context_t* ctx, float y) {
  if (!ctx) return;
  warp_context_set_screen_scroll(ctx, ctx->current_screen, y);
}

int warp_context_get_content_height(warp_context_t* ctx) {
  if (!ctx) return 0;
  int h = 0;
  warp_context_get_screen_svg(ctx, ctx->current_screen, &h);
  return h;
}
