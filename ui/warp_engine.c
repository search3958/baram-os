#include "warp_engine.h"
#include <stddef.h>

#define MAX_VARS 64
#define MAX_NODES 256
#define MAX_TEXTS 128
#define MAX_TOKENS 2048

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
  if (*s == '-') {
    sign = -1;
    s++;
  }
  while (*s >= '0' && *s <= '9') {
    res = res * 10 + (*s - '0');
    s++;
  }
  return res * sign;
}

static char *append_uint(char *p, unsigned int v) {
  char tmp[10];
  int n = 0;
  if (v == 0) {
    *p++ = '0';
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
  if (v < 0) {
    *p++ = '-';
    v = -v;
  }
  return append_uint(p, (unsigned int)v);
}

static char *append_fixed3(char *p, float v) {
  int i = (int)v;
  int f = (int)((v - (float)i) * 1000.0f);
  if (f < 0)
    f = -f;
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

struct {
  char key[64];
  char val[512];
} g_state[MAX_VARS];
int g_state_count = 0;

void set_state(const char *key, const char *val) {
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
  for (int i = 0; i < g_state_count; i++) {
    if (warp_strcmp(g_state[i].key, key) == 0)
      return g_state[i].val;
  }
  return "";
}

typedef struct warp_attr {
  char key[32];
  char value[512];
} warp_attr_t;
typedef struct warp_node {
  char tag[32];
  warp_attr_t attrs[16];
  int attrs_count;
  char event_oneclick[256];
  struct warp_node *children[32];
  int children_count;
  int x, y, w, h;
} warp_node_t;

static warp_node_t g_nodes[MAX_NODES];
static int g_nodes_count = 0;
warp_node_t *g_root_node = NULL;

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
      warp_strncat(out, val, max_len - warp_strlen(out) - 1);
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

typedef enum { TK_WORD, TK_STR, TK_PUNCT, TK_EOF } tk_type;
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

static warp_node_t *alloc_node() {
  if (g_nodes_count < MAX_NODES) {
    warp_node_t *n = &g_nodes[g_nodes_count++];
    warp_memset(n, 0, sizeof(warp_node_t));
    return n;
  }
  return NULL;
}

static warp_node_t *parse_node() {
  if (g_token_pos >= g_token_count)
    return NULL;
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
        if (child && node->children_count < 32)
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
        while (g_token_pos < g_token_count) {
          if (g_tokens[g_token_pos].val[0] == ')')
            break;
          if (g_token_pos + 1 < g_token_count &&
              g_tokens[g_token_pos + 1].val[0] == '(')
            break;
          if (g_token_pos + 1 < g_token_count &&
              g_tokens[g_token_pos + 1].val[0] == ':')
            break;
          if (g_tokens[g_token_pos].val[0] == ',') {
            g_token_pos++;
            break;
          }
          if (g_tokens[g_token_pos].type == TK_STR) {
            warp_strcat(expr, "\"");
            warp_strcat(expr, g_tokens[g_token_pos].val);
            warp_strcat(expr, "\"");
          } else
            warp_strcat(expr, g_tokens[g_token_pos].val);
          g_token_pos++;
        }
        if (warp_strcmp(key, "oneClick") == 0 ||
            warp_strcmp(key, "onClick") == 0)
          warp_strcpy(node->event_oneclick, expr);
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

static int layout_node(warp_node_t *node, int px, int py, int limit_w) {
  if (!node)
    return 0;
  node->x = px;
  node->y = py;
  node->w = limit_w;
  int cy = py;
  if (warp_strcmp(node->tag, "screen") == 0) {
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(node->children[i], px + 20, cy, limit_w - 40) + 10;
    node->h = cy - py + 10;
    return node->h;
  } else if (warp_strcmp(node->tag, "Header") == 0) {
    node->h = 60;
    char text[128];
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 20;
      g_texts[g_texts_count].y = py + 15;
      warp_strcpy(g_texts[g_texts_count].text, text);
      g_texts[g_texts_count].color = 0xFF222222;
      g_texts[g_texts_count].size = 28;
      g_texts_count++;
    }
    int cx = px + limit_w - 20;
    for (int i = 0; i < node->children_count; i++) {
      warp_node_t *child = node->children[i];
      int cw = 160;
      cx -= cw;
      layout_node(child, cx, py + 10, cw);
      cx -= 10;
    }
    return node->h;
  } else if (warp_strcmp(node->tag, "card") == 0) {
    cy += 10;
    char title[128];
    eval_attr(node, "text", title, sizeof(title));
    if (title[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 16;
      g_texts[g_texts_count].y = cy + 4;
      warp_strcpy(g_texts[g_texts_count].text, title);
      g_texts[g_texts_count].color = 0xFF000000;
      g_texts[g_texts_count].size = 22;
      g_texts_count++;
      cy += 40;
    } else
      cy += 10;
    int cx = px + 16;
    for (int i = 0; i < node->children_count; i++)
      cy += layout_node(node->children[i], cx, cy, limit_w - 32) + 16;
    node->h = cy - py + 10;
    if (node->h < 40)
      node->h = 40;
    return node->h;
  } else if (warp_strcmp(node->tag, "text") == 0) {
    char text[512];
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 4;
      g_texts[g_texts_count].y = py + 4;
      warp_strcpy(g_texts[g_texts_count].text, text);
      g_texts[g_texts_count].color = 0xFF333333;
      g_texts[g_texts_count].size = 18;
      g_texts_count++;
    }
    node->h = 24;
    int len = warp_strlen(text);
    if (len > 40)
      node->h = (len / 40 + 1) * 24;
    return node->h;
  } else if (warp_strcmp(node->tag, "button") == 0) {
    node->h = 44;
    char text[128];
    eval_attr(node, "text", text, sizeof(text));
    if (text[0] != '\0' && g_texts_count < MAX_TEXTS) {
      g_texts[g_texts_count].x = px + 16;
      g_texts[g_texts_count].y = py + 11;
      warp_strcpy(g_texts[g_texts_count].text, text);
      g_texts[g_texts_count].color = 0xFFFFFFFF;
      g_texts[g_texts_count].size = 18;
      g_texts_count++;
    }
    return node->h;
  }
  node->h = 20;
  return node->h;
}
/* 文字列をコピーし、コピー後の末尾（'\0'の位置）のポインタを返すヘルパー */
static char *warp_stpcpy(char *dest, const char *src) {
  while ((*dest = *src)) {
    dest++;
    src++;
  }
  return dest;
}

/* 比例定数テーブル (H=40 モデル基準) */
static const float K_X[] = {1.498f,  3.381f,  7.456f, 12.630f,
                            17.368f, 21.770f, 30.573f};
static const float K_Y[] = {0.800f,  3.600f,  7.370f, 12.544f,
                            16.619f, 18.502f, 20.000f};

static void emit_squircle_shape(int x, int y, int w, int h, float radius,
                                const char *fill, const char *extra) {
  float fw = (float)w, fh = (float)h;
  float fx = (float)x, fy = (float)y;

  /* スケール判定 (H=46以上なら 1.15倍モデルを使用) */
  float s = (fh >= 46.0f) ? 1.15f : 1.0f;

  /* 境界点の計算 */
  float edge_x = K_X[6] * s;
  float edge_y = K_Y[6] * s;

  /* 特定の地点での変化（ピンチ処理） */
  if (edge_x > fw / 2.0f)
    edge_x = fw / 2.0f;
  if (edge_y > fh / 2.0f)
    edge_y = fh / 2.0f;

  char buf[8192];
  char *p = buf;

  /* パス開始：右辺の中央 */
  p = warp_stpcpy(p, "<path d=\"M ");
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + fh / 2.0f);

  /* --- 第1コーナー：右下 --- */
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - K_X[0] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - K_X[1] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - K_X[2] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - K_X[3] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - K_X[4] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - K_X[5] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - edge_x);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);

  /* --- 第2コーナー：左下 --- */
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + edge_x);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[5]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);
  *p++ = ' ';
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[4]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh);
  *p++ = ' ';
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[3]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[5] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[2]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[4] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[1]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[3] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + edge_x - (K_X[6] - K_X[0]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[2] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[1] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y + K_Y[0] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + fh - edge_y);

  /* --- 第3コーナー：左上 --- */
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[0] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[1] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + K_X[0] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[2] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + K_X[1] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[3] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + K_X[2] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[4] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + K_X[3] * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[5] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + K_X[4] * s);
  *p++ = ',';
  p = append_fixed3(p, fy);
  *p++ = ' ';
  p = append_fixed3(p, fx + K_X[5] * s);
  *p++ = ',';
  p = append_fixed3(p, fy);
  *p++ = ' ';
  p = append_fixed3(p, fx + edge_x);
  *p++ = ',';
  p = append_fixed3(p, fy);

  /* --- 第4コーナー：右上 --- */
  p = warp_stpcpy(p, " L ");
  p = append_fixed3(p, fx + fw - edge_x);
  *p++ = ',';
  p = append_fixed3(p, fy);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[5]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[4]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[3]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[5] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[2]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[4] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[1]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[3] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw - edge_x + (K_X[6] - K_X[0]) * s);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[2] * s);
  p = warp_stpcpy(p, " C ");
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[1] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y - K_Y[0] * s);
  *p++ = ' ';
  p = append_fixed3(p, fx + fw);
  *p++ = ',';
  p = append_fixed3(p, fy + edge_y);

  /* 閉じ */
  p = warp_stpcpy(p, " Z\" fill=\"");
  p = warp_stpcpy(p, fill);
  p = warp_stpcpy(p, "\" ");
  p = warp_stpcpy(p, extra);
  p = warp_stpcpy(p, " />\n");

  warp_strncat(g_svg_output, buf,
               sizeof(g_svg_output) - warp_strlen(g_svg_output) - 1);
}
static void emit_svg(warp_node_t *node) {
  if (!node)
    return;
  if (warp_strcmp(node->tag, "screen") == 0)
    emit_squircle_shape(0, 0, 1280, 720, 0, "#f5f5f5", "");
  else if (warp_strcmp(node->tag, "Header") == 0)
    emit_squircle_shape(node->x, node->y, node->w, node->h, 0, "#ffffff",
                        "opacity=\"0.8\"");
  else if (warp_strcmp(node->tag, "card") == 0) {
    char color[64], gradient[128], rad_str[32];
    eval_attr(node, "color", color, sizeof(color));
    eval_attr(node, "gradient", gradient, sizeof(gradient));
    eval_attr(node, "radius", rad_str, sizeof(rad_str));
    float radius = (rad_str[0] != '\0') ? (float)warp_strtol(rad_str) : 32.0f;
    const char *fill = "#ffffff";
    if (warp_strcmp(color, "black") == 0)
      fill = "#222222";
    char extra[256] = "stroke=\"#dddddd\" stroke-width=\"1\"";
    if (warp_strncmp(gradient, "conic", 5) == 0) {
      warp_strcat(extra, " id=\"");
      warp_strcat(extra, gradient);
      warp_strcat(extra, "\"");
    }
    emit_squircle_shape(node->x, node->y, node->w, node->h, radius, fill,
                        extra);
    if (warp_strcmp(color, "black") == 0) {
      for (int i = 0; i < g_texts_count; i++) {
        if (g_texts[i].x >= node->x && g_texts[i].x <= node->x + node->w &&
            g_texts[i].y >= node->y && g_texts[i].y <= node->y + node->h)
          g_texts[i].color = 0xFFFFFFFF;
      }
    }
  } else if (warp_strcmp(node->tag, "button") == 0)
    emit_squircle_shape(node->x, node->y, node->w, node->h, -1.0f, "#007aff",
                        "");
  for (int i = 0; i < node->children_count; i++)
    emit_svg(node->children[i]);
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
  if (g_root_node)
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
  warp_memset(g_texts, 0, sizeof(g_texts));
  g_root_node = NULL;
  if (!code || !code[0]) {
    warp_strcpy(g_engine_status, "Err: No Code");
    return;
  }
  g_src_ptr = code;
  while (1) {
    token_t tk = next_token();
    if (tk.type == TK_EOF)
      break;
    if (g_token_count >= MAX_TOKENS)
      break;
    g_tokens[g_token_count++] = tk;
  }
  g_token_pos = 0;
  g_root_node = parse_node();
  if (g_root_node)
    init_state_from_ast(g_root_node);
  update_status_info();
  warp_engine_update(1280, 720);
}

void warp_engine_update(int width, int height) {
  g_texts_count = 0;
  g_svg_output[0] = '\0';
  char w_str[16], h_str[16];
  append_int(w_str, width);
  append_int(h_str, height);
  warp_strcat(g_svg_output, "<svg width=\"");
  warp_strcat(g_svg_output, w_str);
  warp_strcat(g_svg_output, "\" height=\"");
  warp_strcat(g_svg_output, h_str);
  warp_strcat(g_svg_output, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");
  if (g_root_node) {
    layout_node(g_root_node, 0, 0, width);
    emit_svg(g_root_node);
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

extern void sys_restart(void);
static void execute_action(const char *action) {
  if (!action || !action[0])
    return;
  if (warp_strcmp(action, "restart(now)") == 0) {
    sys_restart();
    return;
  }
  if (warp_strncmp(action, "--", 2) == 0) {
    char *eq = warp_strchr(action, '=');
    if (eq) {
      char key[64], val_expr[256], val[512];
      int len = eq - action;
      if (len >= 64)
        len = 63;
      warp_strncpy(key, action, len);
      key[len] = '\0';
      warp_strcpy(val_expr, eq + 1);
      eval_expr(val_expr, val, sizeof(val));
      set_state(key, val);
    }
  }
}

static void check_clicks(warp_node_t *node, int x, int y) {
  if (!node)
    return;
  if (x >= node->x && x <= node->x + node->w && y >= node->y &&
      y <= node->y + node->h) {
    if (node->event_oneclick[0] != '\0')
      execute_action(node->event_oneclick);
  }
  for (int i = 0; i < node->children_count; i++)
    check_clicks(node->children[i], x, y);
}

void warp_engine_click(int x, int y) {
  if (g_root_node) {
    check_clicks(g_root_node, x, y);
    warp_engine_update(1280, 720);
  }
}
