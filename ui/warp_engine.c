#include "warp_engine.h"
#include <stddef.h>

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

static char *warp_strcat(char *dest, const char *src) {
  char *d = dest;
  while (*d)
    d++;
  while ((*d++ = *src++))
    ;
  return dest;
}

static char *warp_strncat(char *dest, const char *src, size_t n) {
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

static char *warp_strchr(const char *s, int c) {
  while (*s != (char)c) {
    if (!*s++)
      return NULL;
  }
  return (char *)s;
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

static char *append_int(char *p, int v) {
  unsigned int uv;
  if (v < 0) {
    *p++ = '-';
    uv = (unsigned int)-(v + 1) + 1;
  } else {
    uv = (unsigned int)v;
  }
  return append_uint(p, uv);
}

static char *append_fixed3(char *p, float v) {
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

struct {
  char key[64];
  char val[512];
} g_state[MAX_VARS];
int g_state_count = 0;

char g_current_screen[64] = "main";

typedef struct {
  char id[64];
  int visible;
} visibility_t;
visibility_t g_visibility[MAX_VARS];
int g_visibility_count = 0;

void set_state(const char *key, const char *val) {
  if (warp_strcmp(key, "_currentScreen") == 0) {
    warp_strncpy(g_current_screen, val, 63);
    return;
  }
  for (int i = 0; i < g_state_count; i++) {
    if (warp_strcmp(g_state[i].key, key) == 0) {
      warp_strncpy(g_state[i].val, val, 511);
      return;
    }
  }
  if (g_state_count < MAX_VARS) {
    warp_strncpy(g_state[g_state_count].key, key, 63);
    warp_strncpy(g_state[g_state_count].val, val, 511);
    g_state_count++;
  }
}

const char *get_state(const char *key) {
  if (warp_strcmp(key, "_currentScreen") == 0)
    return g_current_screen;
  for (int i = 0; i < g_state_count; i++) {
    if (warp_strcmp(g_state[i].key, key) == 0)
      return g_state[i].val;
  }
  return "";
}

void set_visibility(const char *id, int visible) {
  for (int i = 0; i < g_visibility_count; i++) {
    if (warp_strcmp(g_visibility[i].id, id) == 0) {
      g_visibility[i].visible = visible;
      return;
    }
  }
  if (g_visibility_count < MAX_VARS) {
    warp_strncpy(g_visibility[g_visibility_count].id, id, 63);
    g_visibility[g_visibility_count].visible = visible;
    g_visibility_count++;
  }
}

int get_visibility(const char *id) {
  if (!id || id[0] == '\0')
    return 1;
  for (int i = 0; i < g_visibility_count; i++) {
    if (warp_strcmp(g_visibility[i].id, id) == 0)
      return g_visibility[i].visible;
  }
  return 1;
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

static warp_node_t g_nodes[MAX_NODES];
static int g_nodes_count = 0;
warp_node_t *g_root_node = NULL;
static warp_node_t *g_root_nodes[16];
static int g_root_nodes_count = 0;

static script_t g_scripts[MAX_SCRIPTS];
static int g_scripts_count = 0;

typedef struct {
  char target_id[64];
  warp_node_t *nodes[16];
  int node_count;
} dynamic_nodes_t;
static dynamic_nodes_t g_dynamic_nodes[MAX_DYNAMIC_NODES];
static int g_dynamic_nodes_count = 0;

static warp_node_t *alloc_node() {
  if (g_nodes_count < MAX_NODES) {
    warp_node_t *n = &g_nodes[g_nodes_count++];
    warp_memset(n, 0, sizeof(warp_node_t));
    return n;
  }
  return NULL;
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

static void eval_expr(const char *expr, char *out, int max_len) {
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
    } else if (warp_strncmp(p, "--", 2) == 0) {
      char var[64];
      int i = 0;
      while (*p && *p != '\"' && *p != '+' && *p != ' ' && *p != ')' &&
             *p != ',' && i < 63)
        var[i++] = *p++;
      var[i] = '\0';
      const char *val = get_state(var);
      int remaining = max_len - warp_strlen(out) - 1;
      if (remaining > 0)
        warp_strncat(out, val, remaining);
    } else
      p++;
  }
}

static void eval_attr(warp_node_t *node, const char *key, char *buf, int size) {
  const char *val = get_attr(node, key);
  if (val[0] == '\0') {
    buf[0] = '\0';
    return;
  }
  eval_expr(val, buf, size);
}

typedef enum { TK_WORD, TK_STR, TK_PUNCT, TK_AT, TK_EOF } tk_type;
typedef struct {
  tk_type type;
  char val[512];
} token_t;
static const char *g_src_ptr;

static token_t next_token() {
  token_t tk;
  tk.type = TK_EOF;
  tk.val[0] = 0;
  while (*g_src_ptr == ' ' || *g_src_ptr == '\n' || *g_src_ptr == '\r' ||
         *g_src_ptr == '\t')
    g_src_ptr++;
  if (!*g_src_ptr)
    return tk;
  if (*g_src_ptr == '@') {
    tk.type = TK_AT;
    tk.val[0] = *g_src_ptr++;
    tk.val[1] = 0;
    return tk;
  }
  if (*g_src_ptr == '\"' || *g_src_ptr == '\'') {
    char quote = *g_src_ptr++;
    int i = 0;
    tk.type = TK_STR;
    while (*g_src_ptr && *g_src_ptr != quote && i < 511) {
      if (*g_src_ptr == '\\') {
        tk.val[i++] = *g_src_ptr++;
        if (*g_src_ptr)
          tk.val[i++] = *g_src_ptr++;
        continue;
      }
      tk.val[i++] = *g_src_ptr++;
    }
    if (*g_src_ptr == quote)
      g_src_ptr++;
    tk.val[i] = 0;
    return tk;
  }
  if (*g_src_ptr == '(' || *g_src_ptr == ')' || *g_src_ptr == ':' ||
      *g_src_ptr == '=' || *g_src_ptr == '+' || *g_src_ptr == ',') {
    tk.type = TK_PUNCT;
    tk.val[0] = *g_src_ptr++;
    tk.val[1] = 0;
    return tk;
  }
  tk.type = TK_WORD;
  int i = 0;
  while (*g_src_ptr && (unsigned char)*g_src_ptr > 32 && *g_src_ptr != '(' &&
         *g_src_ptr != ')' && *g_src_ptr != ':' && *g_src_ptr != '=' &&
         *g_src_ptr != '+' && *g_src_ptr != ',' && *g_src_ptr != '\"' &&
         *g_src_ptr != '\'' && i < 511) {
    tk.val[i++] = *g_src_ptr++;
  }
  tk.val[i] = 0;
  return tk;
}

static token_t g_tokens[MAX_TOKENS];
static int g_token_count = 0;
static int g_token_pos = 0;

static warp_node_t *parse_node();

static void parse_script() {
  if (g_token_pos >= g_token_count || g_tokens[g_token_pos].type != TK_AT)
    return;
  g_token_pos++; // skip @
  if (g_token_pos >= g_token_count || g_tokens[g_token_pos].type != TK_WORD)
    return;
  if (g_scripts_count >= MAX_SCRIPTS)
    return;
  script_t *s = &g_scripts[g_scripts_count++];
  warp_memset(s, 0, sizeof(script_t));
  warp_strncpy(s->name, g_tokens[g_token_pos].val, 63);
  g_token_pos++;
  if (g_token_pos >= g_token_count || g_tokens[g_token_pos].val[0] != '(')
    return;
  g_token_pos++; // skip (
  while (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] != ')') {
    if (warp_strcmp(g_tokens[g_token_pos].val, "if") == 0 ||
        warp_strcmp(g_tokens[g_token_pos].val, "elseIf") == 0) {
      if (s->block_count >= MAX_SCRIPT_BLOCKS)
        break;
      script_block_t *b = &s->blocks[s->block_count++];
      warp_strncpy(b->type, g_tokens[g_token_pos].val, 15);
      g_token_pos++;
      if (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] == ':')
        g_token_pos++;
      char cond[256] = "";
      while (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] != '(') {
        warp_strncat(cond, g_tokens[g_token_pos].val,
                     255 - warp_strlen(cond));
        g_token_pos++;
      }
      warp_strncpy(b->condition, cond, 255);
      if (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] == '(') {
        g_token_pos++; // skip (
        char actions[1024] = "";
        int paren = 1;
        while (g_token_pos < g_token_count && paren > 0) {
          if (g_tokens[g_token_pos].val[0] == '(')
            paren++;
          else if (g_tokens[g_token_pos].val[0] == ')') {
            paren--;
            if (paren == 0)
              break;
          }
          if (g_tokens[g_token_pos].type == TK_STR)
            warp_strncat(actions, "\"", 1023 - warp_strlen(actions));
          warp_strncat(actions, g_tokens[g_token_pos].val,
                       1023 - warp_strlen(actions));
          if (g_tokens[g_token_pos].type == TK_STR)
            warp_strncat(actions, "\"", 1023 - warp_strlen(actions));
          g_token_pos++;
        }
        warp_strncpy(b->actions, actions, 1023);
        if (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] == ')')
          g_token_pos++;
      }
    } else {
      g_token_pos++;
    }
  }
  if (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] == ')')
    g_token_pos++;
}

static warp_node_t *parse_node() {
  if (g_token_pos >= g_token_count)
    return NULL;
  if (g_tokens[g_token_pos].type == TK_AT) {
    parse_script();
    return NULL;
  }
  token_t t = g_tokens[g_token_pos];
  if (g_token_pos + 1 < g_token_count &&
      g_tokens[g_token_pos + 1].val[0] == '(') {
    warp_node_t *node = alloc_node();
    if (!node)
      return NULL;
    warp_strcpy(node->tag, t.val);
    g_token_pos += 2;
    while (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] != ')') {
      token_t current_tk = g_tokens[g_token_pos];
      if (g_token_pos + 1 < g_token_count &&
          g_tokens[g_token_pos + 1].val[0] == '(') {
        warp_node_t *child = parse_node();
        if (child && node->children_count < 64)
          node->children[node->children_count++] = child;
        continue;
      }
      if (g_token_pos + 1 < g_token_count &&
          g_tokens[g_token_pos + 1].val[0] == ':') {
        char key[64];
        warp_strcpy(key, current_tk.val);
        g_token_pos += 2;
        char expr[512];
        expr[0] = '\0';
        int paren = 0;
        while (g_token_pos < g_token_count) {
          if (paren == 0) {
            if (g_tokens[g_token_pos].val[0] == ')')
              break;
            if (g_token_pos + 1 < g_token_count &&
                g_tokens[g_token_pos + 1].val[0] == '(') {
              const char *name = g_tokens[g_token_pos].val;
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
            if (g_token_pos + 1 < g_token_count &&
                g_tokens[g_token_pos + 1].val[0] == ':')
              break;
            if (g_tokens[g_token_pos].val[0] == ',') {
              if (warp_strcmp(key, "oneClick") != 0 &&
                  warp_strcmp(key, "onClick") != 0 &&
                  warp_strcmp(key, "longPress") != 0) {
                g_token_pos++;
                break;
              }
            }
          }
          if (g_tokens[g_token_pos].val[0] == '(')
            paren++;
          else if (g_tokens[g_token_pos].val[0] == ')')
            paren--;

          if (g_tokens[g_token_pos].type == TK_STR) {
            warp_strncat(expr, "\"", 511 - warp_strlen(expr));
            warp_strncat(expr, g_tokens[g_token_pos].val,
                         511 - warp_strlen(expr));
            warp_strncat(expr, "\"", 511 - warp_strlen(expr));
          } else
            warp_strncat(expr, g_tokens[g_token_pos].val,
                         511 - warp_strlen(expr));
          g_token_pos++;
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
      g_token_pos++;
    }
    if (g_token_pos < g_token_count && g_tokens[g_token_pos].val[0] == ')')
      g_token_pos++;
    return node;
  }
  g_token_pos++;
  return NULL;
}

static void init_state_from_ast(warp_node_t *node) {
  if (!node)
    return;
  for (int i = 0; i < node->attrs_count; i++) {
    if (node->attrs[i].key[0] == '-' && node->attrs[i].key[1] == '-') {
      char val[512];
      eval_expr(node->attrs[i].value, val, sizeof(val));
      set_state(node->attrs[i].key, val);
    }
  }
  for (int i = 0; i < node->children_count; i++)
    init_state_from_ast(node->children[i]);
}

struct warp_text {
  int x, y;
  char text[512];
  uint32_t color;
  float size;
} g_texts[MAX_TEXTS];
int g_texts_count = 0;
char g_svg_output[65536];

static uint32_t warp_hash(const char *s, uint32_t h) {
  while (*s)
    h = h * 31 + (uint32_t)*s++;
  return h;
}

static uint32_t calc_node_state_hash(warp_node_t *node) {
  uint32_t h = 0;
  h = warp_hash(node->tag, h);
  for (int i = 0; i < node->attrs_count; i++) {
    h = warp_hash(node->attrs[i].key, h);
    h = warp_hash(node->attrs[i].value, h);
    // Evaluate expressions because they might depend on global state
    char val[512];
    eval_expr(node->attrs[i].value, val, sizeof(val));
    h = warp_hash(val, h);
  }
  const char *id = get_attr(node, "id");
  h = warp_hash(id, h);
  int visible = get_visibility(id);
  h = h * 31 + (uint32_t)visible;
  return h;
}

static int g_engine_dirty = 0;

int warp_engine_is_dirty() {
    return g_engine_dirty;
}

static int layout_node(warp_node_t *node, int px, int py, int limit_w) {
  if (!node)
    return 0;

  // Save previous state for diffing
  node->prev_x = node->x;
  node->prev_y = node->y;
  node->prev_w = node->w;
  node->prev_h = node->h;
  node->prev_state_hash = node->state_hash;
  node->state_hash = calc_node_state_hash(node);

  const char *id = get_attr(node, "id");
  if (!get_visibility(id)) {
    node->w = 0;
    node->h = 0;
    if (node->prev_w != 0 || node->prev_h != 0) {
        node->is_dirty = 1;
        g_engine_dirty = 1;
    }
    return 0;
  }

  node->x = px;
  node->y = py;
  node->w = limit_w;
  int cy = py;

  if (warp_strcmp(node->tag, "screen") == 0) {
    int start_y = py;
    for (int i = 0; i < node->children_count; i++) {
      if (warp_strcmp(node->children[i]->tag, "Header") == 0) {
        start_y += 80;
        break;
      }
    }
    cy = start_y;
    for (int i = 0; i < node->children_count; i++) {
      if (warp_strcmp(node->children[i]->tag, "Header") == 0) {
        layout_node(node->children[i], px, py, limit_w);
        continue;
      }
      cy += layout_node(node->children[i], px + 24, cy, limit_w - 48) + 12;
    }
    node->h = cy - py + 24;
  } else if (warp_strcmp(node->tag, "Header") == 0) {
    node->h = 64;
    char text[128];
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 24;
      g_texts[g_texts_count].y = py + 18;
      warp_strcpy(g_texts[g_texts_count].text, text);
      g_texts[g_texts_count].color = 0xFF121212;
      g_texts[g_texts_count].size = 24;
      g_texts_count++;
    }
    int cx = px + limit_w - 24;
    for (int i = 0; i < node->children_count; i++) {
      warp_node_t *child = node->children[i];
      int cw = 120;
      cx -= cw;
      layout_node(child, cx, py + 12, cw);
      cx -= 8;
    }
  } else if (warp_strcmp(node->tag, "card") == 0) {
    cy += 12;
    char title[128];
    eval_attr(node, "text", title, sizeof(title));
    if (title[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 24;
      g_texts[g_texts_count].y = cy + 4;
      warp_strcpy(g_texts[g_texts_count].text, title);
      const char *c_prop = get_attr(node, "color");
      const char *hex = get_color_hex(c_prop);
      g_texts[g_texts_count].color = hex ? 0xFF000000 : 0xFF121212;
      g_texts[g_texts_count].size = 20;
      g_texts_count++;
      cy += 36;
    }
    int cx = px + 24;
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(node->children[i], cx, cy, limit_w - 48) + 8;
    if (id[0] != '\0') {
      for (int i = 0; i < g_dynamic_nodes_count; i++) {
        if (warp_strcmp(g_dynamic_nodes[i].target_id, id) == 0) {
          for (int j = 0; j < g_dynamic_nodes[i].node_count; j++) {
            cy += layout_node(g_dynamic_nodes[i].nodes[j], cx, cy,
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
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 4;
      g_texts[g_texts_count].y = py + 4;
      warp_strcpy(g_texts[g_texts_count].text, text);
      const char *c_prop = get_attr(node, "color");
      const char *hex = get_color_hex(c_prop);
      g_texts[g_texts_count].color = hex ? 0xFF000000 : 0xFF333333;
      g_texts[g_texts_count].size = 18;
      g_texts_count++;
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
    char width_prop[32];
    eval_attr(node, "width", width_prop, sizeof(width_prop));
    if (warp_strcmp(width_prop, "max") == 0)
      node->w = limit_w;
    else
      node->w = 140;
    node->h = 40;
    char text[128];
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 16;
      g_texts[g_texts_count].y = py + 10;
      warp_strcpy(g_texts[g_texts_count].text, text);
      if (warp_strcmp(node->tag, "tonalButton") == 0)
        g_texts[g_texts_count].color = 0xFF121212;
      else
        g_texts[g_texts_count].color = 0xFFFFFFFF;
      g_texts[g_texts_count].size = 16;
      g_texts_count++;
    }
  } else if (warp_strcmp(node->tag, "hStack") == 0) {
    int cx = px;
    int max_h = 0;
    int count = node->children_count;
    if (count > 0) {
      int item_w = (limit_w - (count - 1) * 12) / count;
      for (int i = 0; i < count; i++) {
        int ch = layout_node(node->children[i], cx, py, item_w);
        if (ch > max_h)
          max_h = ch;
        cx += item_w + 12;
      }
    }
    node->h = max_h;
  } else if (warp_strcmp(node->tag, "vStack") == 0) {
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(node->children[i], px, cy, limit_w) + 8;
    if (id[0] != '\0') {
      for (int i = 0; i < g_dynamic_nodes_count; i++) {
        if (warp_strcmp(g_dynamic_nodes[i].target_id, id) == 0) {
          for (int j = 0; j < g_dynamic_nodes[i].node_count; j++) {
            cy += layout_node(g_dynamic_nodes[i].nodes[j], px, cy, limit_w) + 8;
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
    g_engine_dirty = 1;
  } else {
    // If layout/state hasn't changed, but children might be dirty
    node->is_dirty = 0;
  }

  // Text is also part of node state, so state_hash handles it.
  return node->h;
}


static char *warp_stpcpy(char *dest, const char *src) {
  while ((*dest = *src)) {
    dest++;
    src++;
  }
  return dest;
}

static const float K_X[] = {1.498f,  3.381f,  7.456f, 12.630f,
                            17.368f, 21.770f, 30.573f};
static const float K_Y[] = {0.800f,  3.600f,  7.370f, 12.544f,
                            16.619f, 18.502f, 20.000f};

static void emit_squircle_shape_to(char *dest, int dest_size, int x, int y, int w, int h, float radius,
                                const char *fill, const char *extra) {
  float fw = (float)w, fh = (float)h;
  float fx = (float)x, fy = (float)y;
  float s = (fh >= 46.0f) ? 1.15f : 1.0f;
  float edge_x = K_X[6] * s;
  float edge_y = K_Y[6] * s;
  if (edge_x > fw / 2.0f) edge_x = fw / 2.0f;
  if (edge_y > fh / 2.0f) edge_y = fh / 2.0f;
  
  char buf[2048];
  char *p = buf;
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
  p = warp_stpcpy(p, " Z\" fill=\"");
  p = warp_stpcpy(p, fill);
  p = warp_stpcpy(p, "\" ");
  p = warp_stpcpy(p, extra);
  p = warp_stpcpy(p, " />\n");
  warp_strncat(dest, buf, dest_size - warp_strlen(dest) - 1);
}

static void emit_svg_recursive(warp_node_t *node, char *dest, int dest_size) {
  if (!node) return;
  const char *id = get_attr(node, "id");
  if (!get_visibility(id)) return;
  
  if (warp_strcmp(node->tag, "screen") == 0) {
    if (warp_strcmp(id, g_current_screen) != 0) return;
    emit_squircle_shape_to(dest, dest_size, 0, 0, node->w, node->h, 0, "#f5f5f5", "");
  } else if (warp_strcmp(node->tag, "Header") == 0) {
    // Header background and content handled at the end
  } else if (warp_strcmp(node->tag, "card") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : "#ffffff";
    char extra[256] = "stroke=\"#dddddd\" stroke-width=\"1\"";
    emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, 32.0f, fill, extra);
  } else if (warp_strcmp(node->tag, "button") == 0 || warp_strcmp(node->tag, "tonalButton") == 0) {
    const char *c_prop = get_attr(node, "color");
    const char *hex = get_color_hex(c_prop);
    const char *fill = hex ? hex : "#0a56d0";
    if (warp_strcmp(node->tag, "tonalButton") == 0)
      emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, fill, "opacity=\"0.1\"");
    else
      emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, fill, "");
  }

  for (int i = 0; i < node->children_count; i++) {
    if (warp_strcmp(node->children[i]->tag, "Header") != 0)
      emit_svg_recursive(node->children[i], dest, dest_size);
  }
  
  if (id[0] != '\0') {
    for (int i = 0; i < g_dynamic_nodes_count; i++) {
      if (warp_strcmp(g_dynamic_nodes[i].target_id, id) == 0) {
        for (int j = 0; j < g_dynamic_nodes[i].node_count; j++) {
          if (warp_strcmp(g_dynamic_nodes[i].nodes[j]->tag, "Header") != 0)
            emit_svg_recursive(g_dynamic_nodes[i].nodes[j], dest, dest_size);
        }
      }
    }
  }

  for (int i = 0; i < node->children_count; i++) {
    if (warp_strcmp(node->children[i]->tag, "Header") == 0) {
      warp_node_t *h = node->children[i];
      emit_squircle_shape_to(dest, dest_size, h->x, h->y, h->w, h->h, 0, "#f5f5f5", "opacity=\"0.9\"");
      for (int k = 0; k < h->children_count; k++)
        emit_svg_recursive(h->children[k], dest, dest_size);
    }
  }
}

static void emit_svg(warp_node_t *node) {
  emit_svg_recursive(node, g_svg_output, sizeof(g_svg_output));
}

static void emit_squircle_shape(int x, int y, int w, int h, float radius,
                                const char *fill, const char *extra) {
  emit_squircle_shape_to(g_svg_output, sizeof(g_svg_output), x, y, w, h, radius, fill, extra);
}


static char g_engine_status[128] = "Idle";
const char *warp_engine_get_status(void) { return g_engine_status; }

static void update_status_info() {
  char buf[128];
  buf[0] = '\0';
  warp_strcat(buf, "Nodes:");
  char n_str[16];
  append_int(n_str, g_nodes_count);
  warp_strcat(buf, n_str);
  warp_strcat(buf, " Tokens:");
  append_int(n_str, g_token_count);
  warp_strcat(buf, n_str);
  warp_strcat(buf, " State:");
  if (g_root_nodes_count > 0)
    warp_strcat(buf, "OK");
  else
    warp_strcat(buf, "NoRoot");
  warp_strncpy(g_engine_status, buf, 127);
}

void warp_engine_init(const char *code) {
  g_nodes_count = 0;
  g_state_count = 0;
  g_token_count = 0;
  g_token_pos = 0;
  g_scripts_count = 0;
  g_root_nodes_count = 0;
  g_visibility_count = 0;
  g_dynamic_nodes_count = 0;
  warp_memset(g_texts, 0, sizeof(g_texts));
  g_root_node = NULL;
  warp_strcpy(g_current_screen, "main");
  if (!code || !code[0]) {
    warp_strcpy(g_engine_status, "Err: No Code");
    return;
  }
  g_src_ptr = code;
  while (1) {
    token_t tk = next_token();
    if (tk.type == TK_EOF || g_token_count >= MAX_TOKENS)
      break;
    g_tokens[g_token_count++] = tk;
  }
  g_token_pos = 0;
  while (g_token_pos < g_token_count) {
    warp_node_t *node = parse_node();
    if (node && g_root_nodes_count < 16)
      g_root_nodes[g_root_nodes_count++] = node;
  }
  if (g_root_nodes_count > 0) {
    for (int i = 0; i < g_root_nodes_count; i++) {
      init_state_from_ast(g_root_nodes[i]);
      if (i == 0 && warp_strcmp(g_root_nodes[i]->tag, "screen") == 0) {
        const char *id = get_attr(g_root_nodes[i], "id");
        if (id[0] != '\0')
          warp_strncpy(g_current_screen, id, 63);
      }
    }
  }
  update_status_info();
  warp_engine_update(1280, 720);
}

static char g_node_svg_buf[4096];

void warp_engine_clear_dirty(void) {
    g_engine_dirty = 0;
    for (int i = 0; i < g_nodes_count; i++) {
        g_nodes[i].is_dirty = 0;
    }
}

int warp_engine_get_node_count(void) {
    return g_nodes_count;
}

void warp_engine_get_node_info(int index, int* x, int* y, int* w, int* h, int* is_dirty) {
    if (index < 0 || index >= g_nodes_count) return;
    *x = g_nodes[index].x;
    *y = g_nodes[index].y;
    *w = g_nodes[index].w;
    *h = g_nodes[index].h;
    *is_dirty = g_nodes[index].is_dirty;
}

void warp_engine_get_node_prev_rect(int index, int* x, int* y, int* w, int* h) {
    if (index < 0 || index >= g_nodes_count) return;
    *x = g_nodes[index].prev_x;
    *y = g_nodes[index].prev_y;
    *w = g_nodes[index].prev_w;
    *h = g_nodes[index].prev_h;
}

const char* warp_engine_get_node_svg(int index) {
    if (index < 0 || index >= g_nodes_count) return "";
    warp_node_t *node = &g_nodes[index];
    
    char w_str[16], h_str[16];
    append_int(w_str, node->w);
    append_int(h_str, node->h);
    
    warp_strcpy(g_node_svg_buf, "<svg width=\"");
    warp_strcat(g_node_svg_buf, w_str);
    warp_strcat(g_node_svg_buf, "\" height=\"");
    warp_strcat(g_node_svg_buf, h_str);
    warp_strcat(g_node_svg_buf, "\" viewBox=\"");
    append_int(w_str, node->x); warp_strcat(g_node_svg_buf, w_str); warp_strcat(g_node_svg_buf, " ");
    append_int(h_str, node->y); warp_strcat(g_node_svg_buf, h_str); warp_strcat(g_node_svg_buf, " ");
    append_int(w_str, node->w); warp_strcat(g_node_svg_buf, w_str); warp_strcat(g_node_svg_buf, " ");
    append_int(h_str, node->h); warp_strcat(g_node_svg_buf, h_str);
    warp_strcat(g_node_svg_buf, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");
    
    emit_svg_recursive(node, g_node_svg_buf, sizeof(g_node_svg_buf));
    
    warp_strcat(g_node_svg_buf, "</svg>");
    return g_node_svg_buf;
}

void warp_engine_update(int width, int height) {
  g_texts_count = 0;
  g_svg_output[0] = '\0';
  g_engine_dirty = 0; // Reset dirty before layout
  
  int total_h = height;
  for (int i = 0; i < g_root_nodes_count; i++) {
    warp_node_t *node = g_root_nodes[i];
    if (warp_strcmp(node->tag, "screen") == 0) {
      const char *id = get_attr(node, "id");
      if (warp_strcmp(id, g_current_screen) != 0)
        continue;
    }
    int h = layout_node(node, 0, 0, width);
    if (h > total_h)
      total_h = h;
  }
  
  char w_str[16], h_str[16];
  append_int(w_str, width);
  append_int(h_str, total_h);
  warp_strcat(g_svg_output, "<svg width=\"");
  warp_strcat(g_svg_output, w_str);
  warp_strcat(g_svg_output, "\" height=\"");
  warp_strcat(g_svg_output, h_str);
  warp_strcat(g_svg_output, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");

  for (int i = 0; i < g_root_nodes_count; i++) {
    warp_node_t *node = g_root_nodes[i];
    if (warp_strcmp(node->tag, "screen") == 0) {
      const char *id = get_attr(node, "id");
      if (warp_strcmp(id, g_current_screen) != 0)
        continue;
    }
    emit_svg(node);
  }
  warp_strcat(g_svg_output, "</svg>");
  update_status_info();
}

const char *warp_engine_get_svg(void) { return g_svg_output; }
extern void layer_draw_ttf(layer_t *layer, int x, int y, const char *str,
                           float font_size, uint32_t color);
void warp_engine_draw_texts(layer_t *layer, int off_x, int off_y) {
  if (!layer)
    return;
  for (int i = 0; i < g_texts_count; i++) {
    layer_draw_ttf(layer, g_texts[i].x + off_x, g_texts[i].y + off_y,
                   g_texts[i].text, g_texts[i].size, g_texts[i].color);
  }
}

static void execute_action(const char *action);

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

static char *evaluate_rhs(const char *expr, char *out, int max_len) {
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
        warp_strncat(sub_expr, get_state(var), 511 - warp_strlen(sub_expr));
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
  eval_expr(expr, out, max_len);
  return out;
}

static int evaluate_condition(const char *cond) {
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
  eval_expr(left_expr, left_val, 511);
  eval_expr(right_expr, right_val, 511);
  return warp_strcmp(left_val, right_val) == 0;
}

static void execute_script(const char *name) {
  for (int i = 0; i < g_scripts_count; i++) {
    if (warp_strcmp(g_scripts[i].name, name) == 0) {
      int matched_if = 0;
      for (int j = 0; j < g_scripts[i].block_count; j++) {
        script_block_t *b = &g_scripts[i].blocks[j];
        if (warp_strcmp(b->type, "if") == 0) {
          if (evaluate_condition(b->condition)) {
            execute_action(b->actions);
            matched_if = 1;
          }
        } else if (warp_strcmp(b->type, "elseIf") == 0) {
          if (!matched_if && evaluate_condition(b->condition)) {
            execute_action(b->actions);
            matched_if = 1;
          }
        }
      }
      return;
    }
  }
}

extern void sys_restart(void);
static void execute_action(const char *action_str) {
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
      warp_strncpy(g_current_screen, screen, 63);
    } else if (warp_strncmp(act, "show(", 5) == 0) {
      char id[64];
      warp_strncpy(id, act + 5, 63);
      char *end = warp_strchr(id, ')');
      if (end)
        *end = '\0';
      set_visibility(id, 1);
    } else if (warp_strncmp(act, "hide(", 5) == 0) {
      char id[64];
      warp_strncpy(id, act + 5, 63);
      char *end = warp_strchr(id, ')');
      if (end)
        *end = '\0';
      set_visibility(id, 0);
    } else if (warp_strncmp(act, "script(", 7) == 0) {
      char name[64];
      warp_strncpy(name, act + 7, 63);
      char *end = warp_strchr(name, ')');
      if (end)
        *end = '\0';
      execute_script(name);
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
        const char *old_src = g_src_ptr;
        int prev_token_count = g_token_count, prev_token_pos = g_token_pos;
        g_src_ptr = code;
        int start_pos = g_token_count;
        while (1) {
          token_t tk = next_token();
          if (tk.type == TK_EOF || g_token_count >= MAX_TOKENS)
            break;
          g_tokens[g_token_count++] = tk;
        }
        g_token_pos = start_pos;
        warp_node_t *new_node = parse_node();
        g_src_ptr = old_src;
        g_token_count = prev_token_count;
        g_token_pos = prev_token_pos;
        if (new_node) {
          new_node->is_dynamic = 1;
          int found = 0;
          for (int i = 0; i < g_dynamic_nodes_count; i++) {
            if (warp_strcmp(g_dynamic_nodes[i].target_id, target_id) == 0) {
              if (g_dynamic_nodes[i].node_count < 16)
                g_dynamic_nodes[i].nodes[g_dynamic_nodes[i].node_count++] =
                    new_node;
              found = 1;
              break;
            }
          }
          if (!found && g_dynamic_nodes_count < MAX_DYNAMIC_NODES) {
            warp_strncpy(g_dynamic_nodes[g_dynamic_nodes_count].target_id,
                         target_id, 63);
            g_dynamic_nodes[g_dynamic_nodes_count].nodes[0] = new_node;
            g_dynamic_nodes[g_dynamic_nodes_count].node_count = 1;
            g_dynamic_nodes_count++;
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
      for (int i = 0; i < g_dynamic_nodes_count; i++) {
        if (warp_strcmp(g_dynamic_nodes[i].target_id, target_id) == 0) {
          if (tag) {
            for (int j = g_dynamic_nodes[i].node_count - 1; j >= 0; j--) {
              if (warp_strcmp(g_dynamic_nodes[i].nodes[j]->tag, tag) == 0) {
                for (int k = j; k < g_dynamic_nodes[i].node_count - 1; k++)
                  g_dynamic_nodes[i].nodes[k] = g_dynamic_nodes[i].nodes[k + 1];
                g_dynamic_nodes[i].node_count--;
                break;
              }
            }
          } else if (g_dynamic_nodes[i].node_count > 0)
            g_dynamic_nodes[i].node_count--;
          break;
        }
      }
    } else if (warp_strncmp(act, "clr(", 4) == 0) {
      char target_id[64];
      warp_strncpy(target_id, act + 4, 63);
      char *end = warp_strchr(target_id, ')');
      if (end)
        *end = '\0';
      for (int i = 0; i < g_dynamic_nodes_count; i++) {
        if (warp_strcmp(g_dynamic_nodes[i].target_id, target_id) == 0) {
          g_dynamic_nodes[i].node_count = 0;
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
        evaluate_rhs(val_expr, val, sizeof(val));
        set_state(key, val);
      }
    }
  }
}

static void check_clicks(warp_node_t *node, int x, int y) {
  if (!node)
    return;
  const char *id = get_attr(node, "id");
  if (!get_visibility(id))
    return;
  for (int i = node->children_count - 1; i >= 0; i--)
    check_clicks(node->children[i], x, y);
  if (id[0] != '\0') {
    for (int i = 0; i < g_dynamic_nodes_count; i++) {
      if (warp_strcmp(g_dynamic_nodes[i].target_id, id) == 0) {
        for (int j = g_dynamic_nodes[i].node_count - 1; j >= 0; j--)
          check_clicks(g_dynamic_nodes[i].nodes[j], x, y);
      }
    }
  }
  if (x >= node->x && x <= node->x + node->w && y >= node->y &&
      y <= node->y + node->h) {
    if (node->event_oneclick[0] != '\0') {
      execute_action(node->event_oneclick);
      return;
    }
  }
}

void warp_engine_click(int x, int y) {
  for (int i = 0; i < g_root_nodes_count; i++) {
    warp_node_t *node = g_root_nodes[i];
    if (warp_strcmp(node->tag, "screen") == 0) {
      const char *id = get_attr(node, "id");
      if (warp_strcmp(id, g_current_screen) != 0)
        continue;
    }
    check_clicks(node, x, y);
  }
  warp_engine_update(1280, 720);
}
