//こちらはwarp1-新しい方です。
#include "warp1_engine.h"
#include "warp_engine.h"
#include <stddef.h>
#include <stdlib.h>

// --- 1. Internal Utilities ---
static int w1_strlen(const char *s) { int n=0; if(!s) return 0; while(s[n]) n++; return n; }
static char *w1_strcpy(char *d, const char *s) { char *p=d; while((*p++=*s++)); return d; }
static char *w1_strncpy(char *d, const char *s, size_t n) { size_t i; for(i=0;i<n&&s[i];i++) d[i]=s[i]; for(;i<n;i++) d[i]=0; return d; }
static char *w1_strcat(char *d, const char *s) { char *p=d; while(*p) p++; while((*p++=*s++)); return d; }
static char *w1_strncat(char *d, const char *s, size_t n) { char *p=d; while(*p) p++; size_t i; for(i=0;i<n&&s[i];i++) { *p++=s[i]; } *p=0; return d; }
static int w1_strcmp(const char *a, const char *b) { while(*a&&*a==*b){a++;b++;} return *(unsigned char*)a-*(unsigned char*)b; }
static int w1_strncmp(const char *a, const char *b, size_t n) { for(size_t i=0;i<n;i++){if(a[i]!=b[i]||!a[i])return (unsigned char)a[i]-(unsigned char)b[i];} return 0; }
static char *w1_strchr(const char *s, int c) { while(*s!=(char)c){if(!*s++)return NULL;} return (char*)s; }
static char *w1_strstr(const char *haystack, const char *needle) {
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

// --- 2. Type Definitions (Memory Optimized) ---
#undef MAX_NODES
#define MAX_NODES 512
#undef MAX_TOKENS
#define MAX_TOKENS 4096

typedef struct warp1_attr { char key[32]; char value[256]; } warp1_attr_t;
typedef struct warp1_node {
    char tag[32]; warp1_attr_t attrs[16]; int attrs_count;
    char event_oneclick[512]; char event_longpress[512];
    struct warp1_node *children[64]; int children_count;
    int x, y, w, h; int is_dynamic;
    int prev_x, prev_y, prev_w, prev_h; int is_dirty;
} warp1_node_t;

typedef struct { char type[16]; char condition[128]; char actions[1024]; } script_block1_t;
typedef struct { char name[64]; script_block1_t blocks[MAX_SCRIPT_BLOCKS]; int block_count; } script1_t;
typedef enum { TK1_WORD, TK1_STR, TK1_PUNCT, TK1_AT, TK1_EOF } tk1_type;
typedef struct { tk1_type type; char val[512]; } token1_t;

struct warp1_context {
    struct { char key[64]; char val[512]; } state[MAX_VARS]; int state_count;
    char current_screen[64];
    warp1_node_t nodes[MAX_NODES]; int nodes_count;
    warp1_node_t *root_nodes[16]; int root_nodes_count;
    script1_t scripts[MAX_SCRIPTS]; int scripts_count;
    const char *src_ptr; token1_t tokens[MAX_TOKENS]; int token_count; int token_pos;
    struct { int x, y; char text[512]; uint32_t color; float size; } texts[MAX_TEXTS]; int texts_count;
    char svg_output[65536]; int engine_dirty; char engine_status[128];
    int mouse_x, mouse_y; int win_w, win_h;
    
    int focused_node_idx; // -1: none
    
    // Screen management (separate SVG per screen)
    char screen_ids[MAX_SCREENS][64];
    char screen_svgs[MAX_SCREENS][65536];
    int screen_content_heights[MAX_SCREENS];
    float screen_scroll_ys[MAX_SCREENS];
    int screen_count;
};

static int w1_tolower(int c) { if (c >= 'A' && c <= 'Z') return c + ('a' - 'A'); return c; }
static int w1_strcasecmp(const char *s1, const char *s2) { while (*s1 && (w1_tolower(*s1) == w1_tolower(*s2))) { s1++; s2++; } return w1_tolower(*s1) - w1_tolower(*s2); }

// --- 3. Global State ---
// set_w1_global / get_w1_global are now defined in kernel.c and provided via warp_engine.h

// --- 4. Logic & Parser ---
static void set_state(warp1_context_t *ctx, const char *key, const char *val) {
    if (w1_strncmp(key, "~~", 2) == 0 || w1_strncmp(key, "--", 2) == 0) {
        // 特殊な接頭辞の処理
        if (w1_strncmp(key, "~~dev/pointerCheck", 18) == 0) {
            // ここでカーネル側のフラグを更新するような仕組みが必要
            // 現状はグローバル変数に保存するのみ
            set_w1_global(key, val);
            return;
        }
        set_w1_global(key, val);
        return;
    }
    if (w1_strcasecmp(key, "_currentScreen") == 0) { w1_strncpy(ctx->current_screen, val, 63); return; }
    for (int i = 0; i < ctx->state_count; i++) {
        if (w1_strcasecmp(ctx->state[i].key, key) == 0) { w1_strncpy(ctx->state[i].val, val, 511); return; }
    }
    if (ctx->state_count < MAX_VARS) {
        w1_strncpy(ctx->state[ctx->state_count].key, key, 63);
        w1_strncpy(ctx->state[ctx->state_count].val, val, 511); ctx->state_count++;
    }
}

static const char *get_state(warp1_context_t *ctx, const char *key) {
    if (w1_strncmp(key, "~~", 2) == 0) {
        // ~~dev/pointerCheck などの値をグローバルから取得
        return get_w1_global(key);
    }
    if (w1_strncmp(key, "--", 2) == 0) return get_w1_global(key);
    if (w1_strcasecmp(key, "_currentScreen") == 0) return ctx->current_screen;
    for (int i = 0; i < ctx->state_count; i++) { if (w1_strcasecmp(ctx->state[i].key, key) == 0) return ctx->state[i].val; }
    // 接頭辞がない場合もグローバルから探す
    const char *global_val = get_w1_global(key);
    if (global_val && global_val[0]) return global_val;
    return "";
}

static long w1_strtol(const char *s) {
    long res = 0; int sign = 1; while (*s == ' ' || *s == '\t') s++;
    if (*s == '-') { sign = -1; s++; }
    while (*s >= '0' && *s <= '9') { res = res * 10 + (*s - '0'); s++; }
    return res * sign;
}

static long eval_math1(const char *s) {
    const char *p = s; while (*p == ' ' || *p == '\t') p++;
    if (!*p) return 0;
    long res = w1_strtol(p);
    while (*p && (*p == ' ' || *p == '\t' || *p == '-' || (*p >= '0' && *p <= '9'))) p++;
    while (*p) {
        while (*p == ' ' || *p == '\t') p++;
        if (!*p) break;
        char op = *p++; 
        while (*p == ' ' || *p == '\t') p++;
        long v = w1_strtol(p);
        if (op == '+') res += v; 
        else if (op == '-') res -= v;
        else if (op == '*') res *= v; 
        else if (op == '/' && v != 0) res /= v;
        while (*p && (*p == ' ' || *p == '\t' || *p == '-' || (*p >= '0' && *p <= '9'))) p++;
    }
    return res;
}

static void eval_expr(warp1_context_t *ctx, const char *expr, char *out, int max_len) {
    out[0] = '\0'; const char *p = expr;
    while (*p) {
        while (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t') p++;
        if (!*p) break;

        if (*p == '+') {
            p++;
            continue; 
        }

        if (*p == '\"' || *p == '\'') {
            char quote = *p++; 
            while (*p && *p != quote) {
                int len = w1_strlen(out);
                if (len < max_len - 1) {
                    if (*p == '\\') {
                        p++; 
                        if (*p == 'n') out[len] = '\n'; 
                        else if (*p == '\"') out[len] = '\"';
                        else if (*p == '\'') out[len] = '\''; 
                        else if (*p == '\\') out[len] = '\\';
                        else { out[len] = *p; }
                        out[len + 1] = '\0'; 
                        if (*p) p++; 
                        continue;
                    }
                    out[len] = *p; 
                    out[len + 1] = '\0';
                }
                p++;
            }
            if (*p == quote) p++;
        } else if (w1_strncmp(p, "--", 2) == 0 || w1_strncmp(p, "~~", 2) == 0) {
            char var[64]; int i = 0;
            while (*p && *p != '\"' && *p != '\'' && *p != '+' && *p != ' ' && *p != ')' && *p != ',' && *p != '}' && i < 63)
                var[i++] = *p++;
            var[i] = '\0'; 
            const char *val = get_state(ctx, var);
            int rem = max_len - w1_strlen(out) - 1; 
            if (rem > 0) w1_strncat(out, val, (size_t)rem);
        } else {
            char word[256]; int i = 0;
            // 単語（未クオートの文字列）として空白を含めて読み込む。
            // ただし、'+' や '}' などの制御記号で止める。
            while (*p && *p != '\"' && *p != '\'' && *p != '+' && *p != ')' && *p != ',' && *p != '}' && i < 255)
                word[i++] = *p++;
            word[i] = '\0';
            
            // 末尾の空白を削除（トークン間の空白は無視したいため）
            int wi = i - 1;
            while (wi >= 0 && (word[wi] == ' ' || word[wi] == '\t' || word[wi] == '\n' || word[wi] == '\r')) {
                word[wi] = '\0';
                wi--;
            }

            if (word[0] == '\0' && *p) { 
                // 何も読み込めなかったが文字がある場合は1文字スキップして無限ループ防止
                p++; 
            } else if (w1_strcmp(word, "null") != 0) { 
                int rem = max_len - w1_strlen(out) - 1; 
                if (rem > 0) w1_strncat(out, word, (size_t)rem); 
            }
        }
    }
}

static const char *get_attr1(warp1_node_t *node, const char *key) {
    for (int i = 0; i < node->attrs_count; i++) {
        if (w1_strcmp(node->attrs[i].key, key) == 0) return node->attrs[i].value;
    }
    return "";
}

static void eval_attr(warp1_context_t *ctx, warp1_node_t *node, const char *key, char *buf, int size) {
    for (int i = 0; i < node->attrs_count; i++) {
        if (w1_strcmp(node->attrs[i].key, key) == 0) { eval_expr(ctx, node->attrs[i].value, buf, size); return; }
    }
    buf[0] = '\0';
}

static void execute_action1(warp1_context_t *ctx, const char *action_str);
static void execute_script1(warp1_context_t *ctx, const char *name) {
    for (int i = 0; i < ctx->scripts_count; i++) {
        if (w1_strcmp(ctx->scripts[i].name, name) == 0) {
            int handled = 0;
            for (int j = 0; j < ctx->scripts[i].block_count; j++) {
                char cond[128]; w1_strcpy(cond, ctx->scripts[i].blocks[j].condition);
                char *eq = w1_strchr(cond, '=');
                if (eq) {
                    *eq = '\0'; char l_val[256], r_val[256];
                    eval_expr(ctx, cond, l_val, 255); eval_expr(ctx, eq+1, r_val, 255);
                    if (w1_strcmp(l_val, r_val) == 0) { execute_action1(ctx, ctx->scripts[i].blocks[j].actions); handled = 1; break; }
                } else if (w1_strcmp(ctx->scripts[i].blocks[j].type, "if") == 0 || !handled) { 
                    execute_action1(ctx, ctx->scripts[i].blocks[j].actions); handled = 1; break; 
                }
            }
            return;
        }
    }
}

static void execute_action1(warp1_context_t *ctx, const char *action_str) {
    if (!action_str || !action_str[0]) return;
    char buf[1024]; w1_strncpy(buf, action_str, 1023); buf[1023] = '\0'; char *p = buf;
    while (*p) {
        char *start = p; int paren = 0, brace = 0, in_q = 0;
        while (*p) {
            if (*p == '\"') in_q = !in_q;
            if (!in_q) {
                if (*p == '(') paren++; else if (*p == ')') paren--;
                else if (*p == '{') brace++; else if (*p == '}') brace--;
                if (*p == ',' && paren == 0 && brace == 0) break;
            }
            p++;
        }
        char act_buf[512]; int len = p - start; if (len >= 512) len = 511;
        w1_strncpy(act_buf, start, len); act_buf[len] = '\0'; if (*p == ',') p++;
        char *act = act_buf; while (*act == ' ' || *act == '\t') act++;

        if (w1_strncmp(act, "reset{", 6) == 0) { extern void sys_restart(void); sys_restart(); }
        else if (w1_strncmp(act, "setScreen{", 10) == 0) {
            char scr[64]; w1_strncpy(scr, act + 10, 63); char *end = w1_strchr(scr, '}');
            if (end) { *end = '\0'; }
            set_state(ctx, "_currentScreen", scr);
        } else if (w1_strncmp(act, "script{", 7) == 0) {
            char sname[64]; w1_strncpy(sname, act + 7, 63); char *end = w1_strchr(sname, '}');
            if (end) { *end = '\0'; } execute_script1(ctx, sname);
        } else if (w1_strncmp(act, "run{", 4) == 0) {
            char expr[256]; w1_strncpy(expr, act + 4, 255); char *end = w1_strchr(expr, '}');
            if (end) { *end = '\0'; }
            char cmd[256];
            eval_expr(ctx, expr, cmd, 255);
            // カーネル側で定義されたコマンド設定関数を呼び出し
            extern void set_pending_command(const char *cmd);
            set_pending_command(cmd);
        } else if (w1_strchr(act, '.')) {
            char *dot = w1_strchr(act, '.'); *dot = '\0'; char *id = act; char *method = dot + 1;
            char *open_b = w1_strchr(method, '{');
            if (open_b) {
                *open_b = '\0'; char *args = open_b + 1; char *close_b = w1_strchr(args, '}');
                if (close_b) { *close_b = '\0'; }
                if (w1_strcmp(method, "changeContent") == 0) {
                    char val[256]; eval_expr(ctx, args, val, 255);
                    char key[128] = "--"; w1_strcat(key, id); w1_strcat(key, "Content"); set_state(ctx, key, val);
                } else if (w1_strcmp(method, "setStatus") == 0) {
                    char key[128] = "--"; w1_strcat(key, id); w1_strcat(key, "Status"); set_state(ctx, key, args);
                }
            }
        } else {
            // 代入文 (var = val) の処理
            char *eq = w1_strchr(act, '='); 
            if (!eq) eq = w1_strchr(act, ':');
            if (eq) {
                *eq = '\0'; 
                char *var_name = act; 
                char *rhs = eq + 1;
                
                // 変数名の前後の空白を削除
                while (*var_name == ' ' || *var_name == '\t' || *var_name == '\n' || *var_name == '\r') var_name++;
                char *v_end = var_name + w1_strlen(var_name) - 1;
                while (v_end >= var_name && (*v_end == ' ' || *v_end == '\t' || *v_end == '\n' || *v_end == '\r')) { *v_end = '\0'; v_end--; }

                // 右辺の前の空白を削除
                while (*rhs == ' ' || *rhs == '\t' || *rhs == '\n' || *rhs == '\r') rhs++;
                
                char val[256];
                if (w1_strncmp(rhs, "calc{", 5) == 0) {
                    char m_expr[256]; w1_strncpy(m_expr, rhs + 5, 255); char *m_end = w1_strchr(m_expr, '}');
                    if (m_end) *m_end = '\0';
                    char m_expanded[512]; eval_expr(ctx, m_expr, m_expanded, 511);
                    long res = eval_math1(m_expanded); extern char *append_int(char *p, int v); append_int(val, (int)res);
                } else if (w1_strstr(rhs, ".replace{")) {
                    char *m_dot = w1_strstr(rhs, ".replace{"); *m_dot = '\0';
                    char base_expr[256], base[256]; w1_strcpy(base_expr, rhs); eval_expr(ctx, base_expr, base, 255);
                    char *m_open = m_dot + 9; char *m_close = w1_strchr(m_open, '}');
                    if (m_close) *m_close = '\0';
                    char old_s[128], new_s[128]; char *comma = w1_strchr(m_open, ',');
                    if (comma) {
                        *comma = '\0'; eval_expr(ctx, m_open, old_s, 127); eval_expr(ctx, comma + 1, new_s, 127);
                        char *found = w1_strstr(base, old_s);
                        if (found && old_s[0]) {
                            int head = found - base; int old_len = w1_strlen(old_s);
                            w1_strncpy(val, base, (size_t)head); val[head] = '\0';
                            w1_strcat(val, new_s); w1_strcat(val, found + old_len);
                        } else { w1_strcpy(val, base); }
                    } else { w1_strcpy(val, base); }
                } else { eval_expr(ctx, rhs, val, 255); }
                
                set_state(ctx, var_name, val);
            }
        }
    }
}

static token1_t next_token(warp1_context_t *ctx) {
    token1_t tk; tk.type = TK1_EOF; tk.val[0] = 0;
    while (*ctx->src_ptr && (unsigned char)*ctx->src_ptr <= 32) ctx->src_ptr++;
    if (!*ctx->src_ptr) return tk;
    if (*ctx->src_ptr == '@') { tk.type = TK1_AT; tk.val[0] = *ctx->src_ptr++; tk.val[1] = 0; return tk; }
    if (*ctx->src_ptr == '\"' || *ctx->src_ptr == '\'') {
        char q = *ctx->src_ptr++; int i = 0; tk.type = TK1_STR;
        while (*ctx->src_ptr && *ctx->src_ptr != q && i < 511) {
            if (*ctx->src_ptr == '\\') { tk.val[i++] = *ctx->src_ptr++; if (*ctx->src_ptr) tk.val[i++] = *ctx->src_ptr++; continue; }
            tk.val[i++] = *ctx->src_ptr++;
        }
        if (*ctx->src_ptr == q) { ctx->src_ptr++; } tk.val[i] = 0; return tk;
    }
    const char *punct = "{}():;=+,";
    for (int i = 0; punct[i]; i++) { if (*ctx->src_ptr == punct[i]) { tk.type = TK1_PUNCT; tk.val[0] = *ctx->src_ptr++; tk.val[1] = 0; return tk; } }
    tk.type = TK1_WORD; int i = 0;
    while (*ctx->src_ptr && (unsigned char)*ctx->src_ptr > 32 && !w1_strchr(punct, *ctx->src_ptr) && i < 511) { tk.val[i++] = *ctx->src_ptr++; }
    tk.val[i] = 0; return tk;
}

static warp1_node_t *alloc_node(warp1_context_t *ctx) {
    if (ctx->nodes_count < MAX_NODES) {
        warp1_node_t *n = &ctx->nodes[ctx->nodes_count++];
        for(int i=0;i<(int)sizeof(warp1_node_t);i++) { ((char*)n)[i] = 0; }
        return n;
    }
    return NULL;
}

static warp1_node_t *parse_node(warp1_context_t *ctx);
static void parse_script(warp1_context_t *ctx) {
    ctx->token_pos++; if (ctx->token_pos >= ctx->token_count) return;
    if (ctx->scripts_count >= MAX_SCRIPTS) { ctx->token_pos++; return; }
    script1_t *s = &ctx->scripts[ctx->scripts_count++]; w1_strncpy(s->name, ctx->tokens[ctx->token_pos].val, 63);
    ctx->token_pos++; if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == '{') {
        ctx->token_pos++;
        while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != '}') {
            if (w1_strcmp(ctx->tokens[ctx->token_pos].val, "if") == 0 || w1_strcmp(ctx->tokens[ctx->token_pos].val, "elseIf") == 0) {
                if (s->block_count >= MAX_SCRIPT_BLOCKS) { ctx->token_pos++; continue; }
                script_block1_t *b = &s->blocks[s->block_count++]; w1_strcpy(b->type, ctx->tokens[ctx->token_pos].val);
                ctx->token_pos++; if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ':') ctx->token_pos++;
                if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == '(') {
                    ctx->token_pos++; int p = 1;
                    while (p > 0 && ctx->token_pos < ctx->token_count) {
                        if (ctx->tokens[ctx->token_pos].val[0] == '(') p++;
                        else if (ctx->tokens[ctx->token_pos].val[0] == ')') p--;
                        if (p > 0) { w1_strcat(b->condition, ctx->tokens[ctx->token_pos].val); ctx->token_pos++; }
                    }
                    if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
                }
                if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == '{') {
                    ctx->token_pos++; int bc = 1;
                    while (bc > 0 && ctx->token_pos < ctx->token_count) {
                        if (ctx->tokens[ctx->token_pos].val[0] == '{') bc++;
                        else if (ctx->tokens[ctx->token_pos].val[0] == '}') bc--;
                        if (bc > 0) {
                            if (ctx->tokens[ctx->token_pos].type == TK1_STR) w1_strcat(b->actions, "\"");
                            w1_strcat(b->actions, ctx->tokens[ctx->token_pos].val);
                            if (ctx->tokens[ctx->token_pos].type == TK1_STR) w1_strcat(b->actions, "\"");
                            ctx->token_pos++;
                        }
                    }
                    if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
                }
            } else { ctx->token_pos++; }
        }
        if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
    }
}

static warp1_node_t *parse_node(warp1_context_t *ctx) {
    if (ctx->token_pos >= ctx->token_count) return NULL;
    if (ctx->tokens[ctx->token_pos].type == TK1_AT) { parse_script(ctx); return NULL; }
    char tag_name[32]; w1_strncpy(tag_name, ctx->tokens[ctx->token_pos].val, 31);
    if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == '{') {
        warp1_node_t *node = alloc_node(ctx); if (!node) { ctx->token_pos++; return NULL; }
        w1_strcpy(node->tag, tag_name); ctx->token_pos += 2;
        while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != '}') {
            if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == '{') {
                warp1_node_t *child = parse_node(ctx);
                if (child && node->children_count < 64) node->children[node->children_count++] = child;
                continue;
            }
            if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == ':') {
                char key[64]; w1_strcpy(key, ctx->tokens[ctx->token_pos].val); ctx->token_pos += 2;
                char expr[512] = "";
                if (ctx->tokens[ctx->token_pos].val[0] == '(') {
                    ctx->token_pos++; int p = 1;
                    while (p > 0 && ctx->token_pos < ctx->token_count) {
                        if (ctx->tokens[ctx->token_pos].val[0] == '(') p++;
                        else if (ctx->tokens[ctx->token_pos].val[0] == ')') p--;
                        if (p > 0) {
                            if (ctx->tokens[ctx->token_pos].type == TK1_STR) w1_strcat(expr, "\"");
                            w1_strcat(expr, ctx->tokens[ctx->token_pos].val);
                            if (ctx->tokens[ctx->token_pos].type == TK1_STR) w1_strcat(expr, "\"");
                            ctx->token_pos++;
                        }
                    }
                    if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
                } else { w1_strcat(expr, ctx->tokens[ctx->token_pos].val); ctx->token_pos++; }
                if (w1_strcmp(key, "oneClick") == 0) { w1_strncpy(node->event_oneclick, expr, 511); }
                else {
                    if (node->attrs_count < 16) {
                        int idx = node->attrs_count++;
                        w1_strncpy(node->attrs[idx].key, key, 31); w1_strncpy(node->attrs[idx].value, expr, 511);
                    }
                }
                if (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] == ',') { ctx->token_pos++; }
                continue;
            }
            ctx->token_pos++;
        }
        if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
        return node;
    }
    ctx->token_pos++; return NULL;
}

// --- 5. Layout & SVG ---
static int layout_node1(warp1_context_t *ctx, warp1_node_t *node, int px, int py, int limit_w) {
    if (!node) return 0;
    node->x = px; node->y = py; node->w = limit_w; int cy = py;
    
    const char *dark_val = get_state(ctx, "~~main/dark");
    int is_dark = (w1_strcmp(dark_val, "true") == 0);

    char frame_v[128], pos_v[128]; eval_attr(ctx, node, "frame", frame_v, 127); eval_attr(ctx, node, "position", pos_v, 127);
    if (frame_v[0]) {
        if (w1_strstr(frame_v, "width")) node->w = (w1_strstr(frame_v, "100vw")) ? ctx->win_w - 40 : 200;
        if (w1_strstr(frame_v, "height")) node->h = 40;
    }
    if (pos_v[0]) {
        if (w1_strstr(pos_v, "bottom")) node->y = ctx->win_h - 60;
        if (w1_strstr(pos_v, "left")) node->x = 20;
    }

    if (w1_strcmp(node->tag, "screen") == 0) {
        for (int i = 0; i < node->children_count; i++) {
            if (w1_strcmp(node->children[i]->tag, "Header") == 0) continue;
            cy += layout_node1(ctx, node->children[i], node->x + 24, cy, limit_w - 48) + 12;
        }
        node->h = cy - py + 24;
    } else if (w1_strcmp(node->tag, "card") == 0) {
        cy += 12; char title[128]; eval_attr(ctx, node, "text", title, 127);
        if (title[0] && ctx->texts_count < MAX_TEXTS) {
            ctx->texts[ctx->texts_count].x = px + 24; ctx->texts[ctx->texts_count].y = cy + 4;
            w1_strcpy(ctx->texts[ctx->texts_count].text, title); ctx->texts[ctx->texts_count].size = 20;
            ctx->texts[ctx->texts_count].color = is_dark ? 0xFFEEEEEE : 0xFF121212; ctx->texts_count++; cy += 36;
        }
        for (int i = 0; i < node->children_count; i++) { cy += layout_node1(ctx, node->children[i], px + 24, cy, limit_w - 48) + 8; }
        node->h = cy - py + 12;
    } else if (w1_strcmp(node->tag, "button") == 0 || w1_strcmp(node->tag, "tonalButton") == 0) {
        node->h = 40; char text[128]; eval_attr(ctx, node, "text", text, 127);
        int text_w = w1_strlen(text) * 9;
        node->w = text_w + 32; if (node->w < 80) node->w = 80;
        if (node->w > limit_w) node->w = limit_w;
        if (ctx->texts_count < MAX_TEXTS) {
            ctx->texts[ctx->texts_count].x = node->x + (node->w - text_w) / 2;
            ctx->texts[ctx->texts_count].y = node->y + 10;
            w1_strcpy(ctx->texts[ctx->texts_count].text, text); ctx->texts[ctx->texts_count].size = 16;
            if (w1_strcmp(node->tag, "tonalButton") == 0) {
                ctx->texts[ctx->texts_count].color = is_dark ? 0xFFFFFFFF : 0xFF000000;
            } else {
                ctx->texts[ctx->texts_count].color = 0xFFFEFFFF;
            }
            ctx->texts_count++;
        }
    
    } else if (w1_strcmp(node->tag, "switch") == 0) {
        // スイッチの標準サイズを定義（44x44）
        node->w = 44;
        node->h = 44;
        return node->h;
    } else if (w1_strcmp(node->tag, "slider") == 0) {
        // スライダーは横幅一杯、高さは操作しやすい32px程度を確保
        node->w = limit_w; 
        node->h = 32; 
    } else if (w1_strcmp(node->tag, "input") == 0) {
        // 入力フォームのボックスサイズを確保
        node->w = limit_w; 
        node->h = 48; 
        
        char out_var[128], val[256], placeholder[128];
        const char *out_var_raw = get_attr1(node, "output");
        if (out_var_raw[0] == '(') {
            w1_strncpy(out_var, out_var_raw + 1, 127);
            char *end = w1_strchr(out_var, ')');
            if (end) *end = '\0';
        } else {
            w1_strncpy(out_var, out_var_raw, 127);
        }

        eval_attr(ctx, node, "placeholder", placeholder, 127);
        
        val[0] = '\0'; 
        if (out_var[0]) { w1_strcpy(val, get_state(ctx, out_var)); }
        
        char id[64], content_key[128]; 
        eval_attr(ctx, node, "id", id, 63);
        if (id[0]) {
            w1_strcpy(content_key, "--"); 
            w1_strcat(content_key, id); 
            w1_strcat(content_key, "Content");
            const char *cv = get_state(ctx, content_key); 
            if (cv[0]) w1_strcpy(val, cv);
        }

        // ボックス内でのテキスト描画位置を調整（中央付近に）
        if (ctx->texts_count < MAX_TEXTS) {
            ctx->texts[ctx->texts_count].x = node->x + 12; 
            ctx->texts[ctx->texts_count].y = node->y + 16; 
            
            if (val[0]) { 
                w1_strcpy(ctx->texts[ctx->texts_count].text, val); 
                // フォーカス中かつ点滅タイミングならカーソルを追加
                int is_focused = 0;
                for (int j = 0; j < ctx->nodes_count; j++) {
                    if (&ctx->nodes[j] == node && ctx->focused_node_idx == j) { is_focused = 1; break; }
                }
                if (is_focused) {
                    const char *ticks_s = get_w1_global("--warpTicks");
                    long ticks = w1_strtol(ticks_s);
                    if ((ticks / 30) % 2 == 0) {
                        w1_strcat(ctx->texts[ctx->texts_count].text, "|");
                    }
                }
                ctx->texts[ctx->texts_count].color = is_dark ? 0xFFCCCCCC : 0xFF333333; 
            } else { 
                w1_strcpy(ctx->texts[ctx->texts_count].text, placeholder); 
                // フォーカス中なら空でもカーソルを表示
                int is_focused = 0;
                for (int j = 0; j < ctx->nodes_count; j++) {
                    if (&ctx->nodes[j] == node && ctx->focused_node_idx == j) { is_focused = 1; break; }
                }
                if (is_focused) {
                    const char *ticks_s = get_w1_global("--warpTicks");
                    long ticks = w1_strtol(ticks_s);
                    if ((ticks / 30) % 2 == 0) {
                        w1_strcpy(ctx->texts[ctx->texts_count].text, "|");
                    }
                }
                ctx->texts[ctx->texts_count].color = is_dark ? 0xFF666666 : 0xFF888888; 
            }
            ctx->texts[ctx->texts_count].size = 16; 
            ctx->texts_count++;
        }
    } else if (w1_strcmp(node->tag, "text") == 0) {
        char text[256]; eval_attr(ctx, node, "text", text, 255);
        if (ctx->texts_count < MAX_TEXTS) {
            ctx->texts[ctx->texts_count].x = px; ctx->texts[ctx->texts_count].y = py;
            w1_strcpy(ctx->texts[ctx->texts_count].text, text); ctx->texts[ctx->texts_count].size = 16;
            ctx->texts[ctx->texts_count].color = is_dark ? 0xFFCCCCCC : 0xFF333333; ctx->texts_count++;
        }
        int lines = 1; for (int i = 0; text[i]; i++) if (text[i] == '\n') lines++;
        node->h = lines * 22;
    } else if (w1_strcmp(node->tag, "hStack") == 0) {
        int cx = px; int max_h = 0; int div = node->children_count ? node->children_count : 1;
        for (int i = 0; i < node->children_count; i++) {
            int h = layout_node1(ctx, node->children[i], cx, py, limit_w / div);
            if (h > max_h) { max_h = h; } cx += node->children[i]->w + 8;
        }
        node->h = max_h;
    } else if (w1_strcmp(node->tag, "vStack") == 0) {
        for (int i = 0; i < node->children_count; i++) { cy += layout_node1(ctx, node->children[i], px, cy, limit_w) + 8; }
        node->h = cy - py;
    } else { for (int i = 0; i < node->children_count; i++) { cy += layout_node1(ctx, node->children[i], px, cy, limit_w) + 4; } node->h = cy - py; }
    return node->h;
}

static void init_state_from_ast1(warp1_context_t *ctx, warp1_node_t *node) {
    if (!node) return;
    for (int i = 0; i < node->attrs_count; i++) {
        if (node->attrs[i].key[0] == '-' && node->attrs[i].key[1] == '-') {
            char val[512]; eval_expr(ctx, node->attrs[i].value, val, 511);
            set_state(ctx, node->attrs[i].key, val);
        }
    }
    for (int i = 0; i < node->children_count; i++) init_state_from_ast1(ctx, node->children[i]);
}

static void emit_rect1(char *dest, int size, int x, int y, int w, int h, const char *fill, const char *extra) {
    char buf[256]; char *p = buf;
    p = w1_strcpy(p, "<rect x=\""); p = append_int(p, x);
    p = w1_strcat(p, "\" y=\""); p = append_int(p, y);
    p = w1_strcat(p, "\" width=\""); p = append_int(p, w);
    p = w1_strcat(p, "\" height=\""); p = append_int(p, h);
    p = w1_strcat(p, "\" fill=\""); p = w1_strcat(p, fill);
    p = w1_strcat(p, "\" "); p = w1_strcat(p, extra);
    p = w1_strcat(p, " />\n");
    w1_strncat(dest, buf, (size_t)(size - w1_strlen(dest) - 1));
}

static void emit_squircle_shape1(char *dest, int size, int x, int y, int w, int h, float radius, const char *fill, const char *extra) {
    // Forward to common engine helper
    extern void emit_squircle_shape_to(char *dest, int dest_size, int x, int y, int w, int h, float radius, const char *fill, const char *extra);
    emit_squircle_shape_to(dest, size, x, y, w, h, radius, fill, extra);
}



static void emit_svg_recursive1(warp1_context_t *ctx, warp1_node_t *node, char *dest, int dest_size) {
    if (!node) return;
    const char *id_attr = ""; 
    for(int i=0;i<node->attrs_count;i++) {
        if(w1_strcmp(node->attrs[i].key, "id")==0) {
            id_attr = node->attrs[i].value;
        }
    }
    
    const char *dark_val = get_state(ctx, "~~main/dark");
    int is_dark = (w1_strcmp(dark_val, "true") == 0);

    if (w1_strcmp(node->tag, "screen") == 0) {
        char real_id[64]; eval_expr(ctx, id_attr, real_id, 63);
        if (w1_strcmp(real_id, ctx->current_screen) != 0) return;
        emit_squircle_shape1(dest, dest_size, 0, 0, node->w, node->h, 0, is_dark ? "#121212" : "#f1f2f2", "");

    } else if (w1_strcmp(node->tag, "card") == 0) {
        emit_squircle_shape1(dest, dest_size, node->x, node->y, node->w, node->h, 32.0f, is_dark ? "#1e1e1e" : "#ffffff", "");

    } else if (w1_strcmp(node->tag, "button") == 0) {
        emit_squircle_shape1(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, "#0A60FF", "");

    } else if (w1_strcmp(node->tag, "tonalButton") == 0) {
        emit_squircle_shape1(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, is_dark ? "#ffffff" : "#000000", "opacity=\"0.1\"");
        
    } else if (w1_strcmp(node->tag, "switch") == 0) {
        // スイッチの描画
        const char *out_var_raw = get_attr1(node, "output");
        char out_var[128]; 
        if (out_var_raw[0] == '(') {
            w1_strncpy(out_var, out_var_raw + 1, 127);
            char *end = w1_strchr(out_var, ')');
            if (end) *end = '\0';
        } else {
            w1_strncpy(out_var, out_var_raw, 127);
        }
        
        const char *val = get_state(ctx, out_var);
        int on = (w1_strstr(val, "true") != NULL);
        int disabled = (w1_strstr(val, "Disabled") != NULL);

        // 背景
        const char *bg_color = on ? "#0A60FF" : "#dddddd";
        if (disabled) bg_color = on ? "#80A0FF" : "#eeeeee"; // 無効時は色を薄く
        int size = 44;
        int x = node->x + (node->w - size) / 2;
        int y = node->y + (node->h - size) / 2;
        emit_squircle_shape1(dest, dest_size, x, y, size, size, 0.0f, bg_color, "");

        // チェックマーク（true の場合のみ）
        if (on) {
            char check_buf[512]; char *p = check_buf;
            p = w1_strcpy(p, "<path d=\"M");
            p = append_int(p, x + 12); p = w1_strcat(p, " ");
            p = append_int(p, y + 22);
            p = w1_strcat(p, " L"); p = append_int(p, x + 20); p = w1_strcat(p, " "); p = append_int(p, y + 30);
            p = w1_strcat(p, " L"); p = append_int(p, x + 34); p = w1_strcat(p, " "); p = append_int(p, y + 14);
            p = w1_strcat(p, "\" stroke=\"#ffffff\" stroke-width=\"4\" fill=\"none\" />\n");
            w1_strncat(dest, check_buf, (size_t)(dest_size - w1_strlen(dest) - 1));
        }
        
    } else if (w1_strcmp(node->tag, "slider") == 0) {
        // スライダーの描画 - squircle を使用
        char val[128]; 
        eval_attr(ctx, node, "status", val, 127);
        int v = (int)w1_strtol(val); 
        if (v < 0) v = 0; 
        if (v > 100) v = 100;
        
        // トラック（背景の線）- 細い角丸矩形
        emit_squircle_shape1(dest, dest_size, node->x, node->y + 14, node->w, 4, 2.0f, "#dddddd", "");
        
        // ノブ（操作する丸）- 完全な円形
        int knob_x = node->x + (node->w * v / 100) - 10;
        if (knob_x < node->x - 10) knob_x = node->x - 10;
        if (knob_x > node->x + node->w - 10) knob_x = node->x + node->w - 10;
        emit_squircle_shape1(dest, dest_size, knob_x, node->y + 6, 20, 20, 10.0f, "#0A60FF", "");
        
    } else if (w1_strcmp(node->tag, "input") == 0) {
        // 入力フォームの描画 - 角丸矩形
        const char *stroke = "#dddddd";
        const char *stroke_w = "1";
        
        // フォーカスされている場合は枠線を強調
        for (int i = 0; i < ctx->nodes_count; i++) {
            if (&ctx->nodes[i] == node && ctx->focused_node_idx == i) {
                stroke = "#0A60FF";
                stroke_w = "2";
                break;
            }
        }
        
        char extra[128];
        w1_strcpy(extra, "stroke=\""); w1_strcat(extra, stroke);
        w1_strcat(extra, "\" stroke-width=\""); w1_strcat(extra, stroke_w); w1_strcat(extra, "\"");
        
        emit_squircle_shape1(dest, dest_size, node->x, node->y, node->w, node->h, 8.0f, "#ffffff", extra);
    }
    
    // 子要素の描画
    for (int i = 0; i < node->children_count; i++) {
        if (w1_strcmp(node->children[i]->tag, "Header") != 0) {
            emit_svg_recursive1(ctx, node->children[i], dest, dest_size);
        }
    }
}

warp1_context_t* warp1_context_create(const char* code) {
    warp1_context_t* ctx = (warp1_context_t*)malloc(sizeof(warp1_context_t)); if (!ctx) return NULL;
    for(int i=0;i<(int)sizeof(warp1_context_t);i++) { ((char*)ctx)[i] = 0; }
    ctx->focused_node_idx = -1;
    ctx->screen_count = 0;
    // システムログを初期化
    ctx->src_ptr = code; while(1) {
        token1_t tk = next_token(ctx); if (tk.type == TK1_EOF || ctx->token_count >= MAX_TOKENS) break;
        ctx->tokens[ctx->token_count++] = tk;
    }
    ctx->token_pos = 0; while (ctx->token_pos < ctx->token_count) {
        warp1_node_t *node = parse_node(ctx); if (node && ctx->root_nodes_count < 16) ctx->root_nodes[ctx->root_nodes_count++] = node;
    }
    if (ctx->root_nodes_count > 0) {
        for (int i = 0; i < ctx->root_nodes_count; i++) init_state_from_ast1(ctx, ctx->root_nodes[i]);
        char id[64]; eval_attr(ctx, ctx->root_nodes[0], "id", id, 63); w1_strcpy(ctx->current_screen, id[0]?id:"main");
    }
    warp1_context_update(ctx, 1280, 720); return ctx;
}

void warp1_context_destroy(warp1_context_t* ctx) { if (ctx) free(ctx); }

void warp1_context_update(warp1_context_t* ctx, int width, int height) {
    ctx->texts_count = 0; ctx->svg_output[0] = '\0'; ctx->win_w = width; ctx->win_h = height;
    int total_h = height;
    for (int i = 0; i < ctx->root_nodes_count; i++) {
        char id[64]; eval_attr(ctx, ctx->root_nodes[i], "id", id, 63);
        if (w1_strcmp(id, ctx->current_screen) != 0) continue;
        int h = layout_node1(ctx, ctx->root_nodes[i], 0, 0, width); if (h > total_h) total_h = h;
    }
    extern char *append_int(char *p, int v); char w_str[16], h_str[16]; append_int(w_str, width); append_int(h_str, total_h);
    w1_strcpy(ctx->svg_output, "<svg width=\""); w1_strcat(ctx->svg_output, w_str); w1_strcat(ctx->svg_output, "\" height=\""); w1_strcat(ctx->svg_output, h_str);
    w1_strcat(ctx->svg_output, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");
    for (int i = 0; i < ctx->root_nodes_count; i++) emit_svg_recursive1(ctx, ctx->root_nodes[i], ctx->svg_output, sizeof(ctx->svg_output));
    w1_strcat(ctx->svg_output, "</svg>");
    
    // Register/update current screen in screen list
    int screen_idx = -1;
    for (int i = 0; i < ctx->screen_count; i++) {
        if (w1_strcmp(ctx->screen_ids[i], ctx->current_screen) == 0) { screen_idx = i; break; }
    }
    if (screen_idx < 0 && ctx->screen_count < MAX_SCREENS) {
        screen_idx = ctx->screen_count++;
        w1_strncpy(ctx->screen_ids[screen_idx], ctx->current_screen, 63);
        ctx->screen_scroll_ys[screen_idx] = 0.0f;
    }
    if (screen_idx >= 0) {
        w1_strncpy(ctx->screen_svgs[screen_idx], ctx->svg_output, 65535);
        ctx->screen_content_heights[screen_idx] = total_h;
    }
}

const char* warp1_context_get_svg(warp1_context_t* ctx) { return ctx->svg_output; }
void warp1_context_draw_texts(warp1_context_t* ctx, layer_t* layer, int ox, int oy) {
    extern void layer_draw_ttf(layer_t *l, int x, int y, const char *s, float sz, uint32_t c);
    for (int i = 0; i < ctx->texts_count; i++) {
        layer_draw_ttf(layer, ctx->texts[i].x + ox, ctx->texts[i].y + oy, ctx->texts[i].text, ctx->texts[i].size, ctx->texts[i].color);
    }
}


void warp1_context_click(warp1_context_t* ctx, int x, int y) {
    int clicked = 0;
    // 逆順（手前に描画されたものから）でチェック
    for (int i = ctx->nodes_count - 1; i >= 0; i--) {
        warp1_node_t *n = &ctx->nodes[i];
        if (x >= n->x && x <= n->x + n->w && y >= n->y && y <= n->y + n->h) {
            clicked = 1;
            
            // フォーカスをリセット。inputをクリックした場合のみ後でセットされる。
            ctx->focused_node_idx = -1;

            if (w1_strcmp(n->tag, "switch") == 0) {
                const char *out_var_raw = get_attr1(n, "output");
                char out_var[128]; 
                // 括弧 "(...)" がある場合は中身を抽出
                if (out_var_raw[0] == '(') {
                    w1_strncpy(out_var, out_var_raw + 1, 127);
                    char *end = w1_strchr(out_var, ')');
                    if (end) *end = '\0';
                } else {
                    w1_strncpy(out_var, out_var_raw, 127);
                }

                if (out_var[0]) {
                    const char *current = get_state(ctx, out_var);
                    // "Disabled"が含まれる場合はクリックさせない
                    if (w1_strstr(current, "Disabled") == NULL) {
                        int on = (w1_strstr(current, "true") != NULL);
                        set_state(ctx, out_var, on ? "false" : "true");
                        if (n->event_oneclick[0]) {
                            execute_action1(ctx, n->event_oneclick);
                        }
                    }
                }
                break;
            }
            if (w1_strcmp(n->tag, "slider") == 0) {
                const char *out_var_raw = get_attr1(n, "output");
                char out_var[128];
                if (out_var_raw[0] == '(') {
                    w1_strncpy(out_var, out_var_raw + 1, 127);
                    char *end = w1_strchr(out_var, ')');
                    if (end) *end = '\0';
                } else {
                    w1_strncpy(out_var, out_var_raw, 127);
                }

                if (out_var[0]) {
                    int val = (x - n->x) * 100 / n->w;
                    if (val < 0) val = 0;
                    if (val > 100) val = 100;
                    char val_str[16];
                    extern char *append_int(char *p, int v);
                    append_int(val_str, val);
                    set_state(ctx, out_var, val_str);
                }
                break;
            }
            if (w1_strcmp(n->tag, "input") == 0) { 
                ctx->focused_node_idx = i;
                break; 
            }
            if (w1_strcmp(n->tag, "button") == 0 || w1_strcmp(n->tag, "tonalButton") == 0) {
                if (n->event_oneclick[0]) {
                    execute_action1(ctx, n->event_oneclick);
                }
                break;
            }
            if (n->event_oneclick[0]) {
                execute_action1(ctx, n->event_oneclick);
                break;
            }
        }
    }
    if (!clicked) {
        ctx->focused_node_idx = -1;
    }
    // 状態変化があった場合のみ更新
    ctx->engine_dirty = 1;
}

void warp1_context_key_input(warp1_context_t* ctx, char c) {
    // デバッグログ: 入力された文字のコードを表示
    char key_msg[32] = "Key: 0x";
    extern char *append_hex8(char *p, uint8_t v);
    append_hex8(key_msg + 7, (uint8_t)c);

    uint8_t uc = (uint8_t)c;
    // 矢印キー（drivers.h の定義: 0x11〜0x14）によるフォーカス移動
    if (uc == 0x11 || uc == 0x13) { // UP or LEFT -> 前の input へ
        int start = (ctx->focused_node_idx <= 0) ? ctx->nodes_count - 1 : ctx->focused_node_idx - 1;
        for (int i = 0; i < ctx->nodes_count; i++) {
            int idx = (start - i + ctx->nodes_count) % ctx->nodes_count;
            if (w1_strcmp(ctx->nodes[idx].tag, "input") == 0) {
                ctx->focused_node_idx = idx;
                break;
            }
        }
        ctx->engine_dirty = 1;
        return;
    }
    if (uc == 0x12 || uc == 0x14 || uc == '\t') { // DOWN or RIGHT or TAB -> 次の input へ
        int start = (ctx->focused_node_idx < 0) ? 0 : ctx->focused_node_idx + 1;
        for (int i = 0; i < ctx->nodes_count; i++) {
            int idx = (start + i) % ctx->nodes_count;
            if (w1_strcmp(ctx->nodes[idx].tag, "input") == 0) {
                ctx->focused_node_idx = idx;
                ctx->engine_dirty = 1;
                return;
            }
        }
    }

    if (ctx->focused_node_idx < 0 || ctx->focused_node_idx >= ctx->nodes_count) return;
    warp1_node_t *n = &ctx->nodes[ctx->focused_node_idx];
    if (w1_strcmp(n->tag, "input") != 0) return;

    const char *out_var_raw = get_attr1(n, "output");
    char out_var[128];
    if (out_var_raw[0] == '(') {
        w1_strncpy(out_var, out_var_raw + 1, 127);
        char *end = w1_strchr(out_var, ')');
        if (end) *end = '\0';
    } else {
        w1_strncpy(out_var, out_var_raw, 127);
    }
    
    if (!out_var[0]) {
        return;
    }

    char val[512];
    const char *current_val = get_state(ctx, out_var);
    w1_strcpy(val, current_val);
    int len = w1_strlen(val);

    char log_tmp[128] = "Input to: ";
    w1_strcat(log_tmp, out_var);

    if (uc == 8 || uc == 127) { // Backspace
        if (len > 0) {
            val[len - 1] = '\0';
            set_state(ctx, out_var, val);
        }
    } else if (c >= 32 && c <= 126) { // Printables
        if (len < 511) {
            val[len] = c;
            val[len + 1] = '\0';
            set_state(ctx, out_var, val);
        }
    }

    ctx->engine_dirty = 1;
}


int warp1_context_is_dirty(warp1_context_t* ctx) { return ctx->engine_dirty; }
void warp1_context_clear_dirty(warp1_context_t* ctx) { ctx->engine_dirty = 0; }
void warp1_context_set_state(warp1_context_t* ctx, const char* k, const char* v) { set_state(ctx, k, v); ctx->engine_dirty = 1; }
void warp1_context_set_mouse(warp1_context_t* ctx, int x, int y) { ctx->mouse_x = x; ctx->mouse_y = y; }
int warp1_context_get_node_count(warp1_context_t* ctx) { return ctx->nodes_count; }
void warp1_context_get_node_info(warp1_context_t* ctx, int index, int* x, int* y, int* w, int* h, int* d) {
    if (index < 0 || index >= ctx->nodes_count) return;
    *x = ctx->nodes[index].x; *y = ctx->nodes[index].y; *w = ctx->nodes[index].w; *h = ctx->nodes[index].h; *d = ctx->nodes[index].is_dirty;
}
const char* warp1_context_get_node_svg(warp1_context_t* ctx, int i) { return ""; }
void warp1_context_get_node_prev_rect(warp1_context_t* ctx, int i, int* x, int* y, int* w, int* h) { (void)i; *x=*y=*w=*h=0; }
const char* warp1_context_get_status(warp1_context_t* ctx) { return ctx->engine_status; }

static warp1_node_t* find_header_node1(warp1_context_t* ctx) {
    for (int i = 0; i < ctx->nodes_count; i++) { if (w1_strcmp(ctx->nodes[i].tag, "Header") == 0) return &ctx->nodes[i]; }
    return NULL;
}

int warp1_context_get_header_info(warp1_context_t* ctx, char* t, int m, int* c) {
    warp1_node_t *h = find_header_node1(ctx); if (!h) return 0;
    eval_attr(ctx, h, "text", t, m); *c = h->children_count; return 1;
}

void warp1_context_get_header_action_info(warp1_context_t* ctx, int i, char* t, int m) {
    warp1_node_t *h = find_header_node1(ctx); if (!h || i < 0 || i >= h->children_count) return;
    eval_attr(ctx, h->children[i], "text", t, m);
}

void warp1_context_click_header_action(warp1_context_t* ctx, int i) {
    warp1_node_t *h = find_header_node1(ctx); if (!h || i < 0 || i >= h->children_count) return;
    if (h->children[i]->event_oneclick[0]) execute_action1(ctx, h->children[i]->event_oneclick);
}

// Screen-based scroll management
const char* warp1_context_get_screen_svg(warp1_context_t* ctx, const char* screen_id, int* content_height) {
    if (!ctx || !screen_id) return NULL;
    for (int i = 0; i < ctx->screen_count; i++) {
        if (w1_strcmp(ctx->screen_ids[i], screen_id) == 0) {
            if (content_height) *content_height = ctx->screen_content_heights[i];
            return ctx->screen_svgs[i];
        }
    }
    return NULL;
}

void warp1_context_set_screen_scroll(warp1_context_t* ctx, const char* screen_id, float scroll_y) {
    if (!ctx || !screen_id) return;
    for (int i = 0; i < ctx->screen_count; i++) {
        if (w1_strcmp(ctx->screen_ids[i], screen_id) == 0) {
            ctx->screen_scroll_ys[i] = scroll_y;
            return;
        }
    }
}

float warp1_context_get_screen_scroll(warp1_context_t* ctx, const char* screen_id) {
    if (!ctx || !screen_id) return 0.0f;
    for (int i = 0; i < ctx->screen_count; i++) {
        if (w1_strcmp(ctx->screen_ids[i], screen_id) == 0) {
            return ctx->screen_scroll_ys[i];
        }
    }
    return 0.0f;
}

// Legacy scroll API (for backward compatibility)
float warp1_context_get_scroll_y(warp1_context_t* ctx) {
    if (!ctx) return 0.0f;
    return warp1_context_get_screen_scroll(ctx, ctx->current_screen);
}

void warp1_context_set_scroll_y(warp1_context_t* ctx, float y) {
    if (!ctx) return;
    warp1_context_set_screen_scroll(ctx, ctx->current_screen, y);
}

float warp1_context_get_target_scroll_y(warp1_context_t* ctx) {
    if (!ctx) return 0.0f;
    return warp1_context_get_screen_scroll(ctx, ctx->current_screen);
}

void warp1_context_set_target_scroll_y(warp1_context_t* ctx, float y) {
    if (!ctx) return;
    warp1_context_set_screen_scroll(ctx, ctx->current_screen, y);
}

int warp1_context_get_content_height(warp1_context_t* ctx) {
    if (!ctx) return 0;
    int h = 0;
    warp1_context_get_screen_svg(ctx, ctx->current_screen, &h);
    return h;
}
