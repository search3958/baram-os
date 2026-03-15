#include "warp1_engine.h"
#include "warp_engine.h"
#include <stddef.h>
#include <stdlib.h>

// Internal Utilities (Moved to top to avoid implicit declaration errors)
static int w1_strlen(const char *s) {
    int n = 0; if (!s) return 0;
    while (s[n]) { n++; }
    return n;
}
static char *w1_strcpy(char *d, const char *s) {
    char *p = d; while ((*p++ = *s++)) { }
    return d;
}
static char *w1_strncpy(char *d, const char *s, size_t n) {
    size_t i; for (i = 0; i < n && s[i]; i++) { d[i] = s[i]; }
    for (; i < n; i++) { d[i] = 0; }
    return d;
}
static char *w1_strcat(char *d, const char *s) {
    char *p = d; while (*p) { p++; }
    while ((*p++ = *s++)) { }
    return d;
}
static char *w1_strncat(char *d, const char *s, size_t n) {
    char *p = d; while (*p) { p++; }
    size_t i; for (i = 0; i < n && s[i]; i++) { *p++ = s[i]; }
    *p = 0; return d;
}
static int w1_strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return *(unsigned char*)a - *(unsigned char*)b;
}
static int w1_strncmp(const char *a, const char *b, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (a[i] != b[i] || !a[i]) return (unsigned char)a[i] - (unsigned char)b[i];
    }
    return 0;
}
static char *w1_strchr(const char *s, int c) {
    while (*s != (char)c) { if (!*s++) return NULL; }
    return (char*)s;
}

// Global variables for ~~ prefix variables
static struct { char key[64]; char val[512]; } g_warp1_globals[MAX_VARS];
static int g_warp1_global_count = 0;

static void set_w1_global(const char *key, const char *val) {
    for (int i = 0; i < g_warp1_global_count; i++) {
        if (w1_strcmp(g_warp1_globals[i].key, key) == 0) {
            w1_strncpy(g_warp1_globals[i].val, val, 511);
            return;
        }
    }
    if (g_warp1_global_count < MAX_VARS) {
        w1_strncpy(g_warp1_globals[g_warp1_global_count].key, key, 63);
        w1_strncpy(g_warp1_globals[g_warp1_global_count].val, val, 511);
        g_warp1_global_count++;
    }
}

static const char *get_w1_global(const char *key) {
    for (int i = 0; i < g_warp1_global_count; i++) {
        if (w1_strcmp(g_warp1_globals[i].key, key) == 0) {
            return g_warp1_globals[i].val;
        }
    }
    return "";
}

// Context & Nodes
typedef struct warp1_attr { char key[32]; char value[512]; } warp1_attr_t;
typedef struct warp1_node {
    char tag[32]; warp1_attr_t attrs[16]; int attrs_count;
    char event_oneclick[512]; char event_longpress[512];
    struct warp1_node *children[64]; int children_count;
    int x, y, w, h; int is_dynamic;
    int prev_x, prev_y, prev_w, prev_h; int is_dirty;
    uint32_t state_hash; uint32_t prev_state_hash;
} warp1_node_t;

typedef struct { char type[16]; char condition[256]; char actions[1024]; } script_block1_t;
typedef struct { char name[64]; script_block1_t blocks[MAX_SCRIPT_BLOCKS]; int block_count; } script1_t;
typedef struct { char id[64]; int visible; } visibility1_t;
typedef struct { char target_id[64]; warp1_node_t *nodes[16]; int node_count; } dynamic_nodes1_t;
typedef enum { TK1_WORD, TK1_STR, TK1_PUNCT, TK1_AT, TK1_EOF } tk1_type;
typedef struct { tk1_type type; char val[512]; } token1_t;

struct warp1_context {
    struct { char key[64]; char val[512]; } state[MAX_VARS]; int state_count;
    char current_screen[64]; visibility1_t visibility[MAX_VARS]; int visibility_count;
    warp1_node_t nodes[MAX_NODES]; int nodes_count;
    warp1_node_t *root_nodes[16]; int root_nodes_count;
    script1_t scripts[MAX_SCRIPTS]; int scripts_count;
    dynamic_nodes1_t dynamic_nodes[MAX_DYNAMIC_NODES]; int dynamic_nodes_count;
    const char *src_ptr; token1_t tokens[MAX_TOKENS]; int token_count; int token_pos;
    struct { int x, y; char text[512]; uint32_t color; float size; } texts[MAX_TEXTS]; int texts_count;
    char svg_output[65536]; int engine_dirty; char engine_status[128]; char node_svg_buf[4096];
    int mouse_x, mouse_y; int win_w, win_h;
};

// State logic
static void set_state(warp1_context_t *ctx, const char *key, const char *val) {
    if (w1_strncmp(key, "~~", 2) == 0) { set_w1_global(key, val); return; }
    if (w1_strcmp(key, "_currentScreen") == 0) { w1_strncpy(ctx->current_screen, val, 63); return; }
    for (int i = 0; i < ctx->state_count; i++) {
        if (w1_strcmp(ctx->state[i].key, key) == 0) {
            w1_strncpy(ctx->state[i].val, val, 511);
            return;
        }
    }
    if (ctx->state_count < MAX_VARS) {
        w1_strncpy(ctx->state[ctx->state_count].key, key, 63);
        w1_strncpy(ctx->state[ctx->state_count].val, val, 511);
        ctx->state_count++;
    }
}

static const char *get_state(warp1_context_t *ctx, const char *key) {
    if (w1_strncmp(key, "~~", 2) == 0) return get_w1_global(key);
    if (w1_strcmp(key, "_currentScreen") == 0) return ctx->current_screen;
    for (int i = 0; i < ctx->state_count; i++) {
        if (w1_strcmp(ctx->state[i].key, key) == 0) return ctx->state[i].val;
    }
    return "";
}

static void eval_expr(warp1_context_t *ctx, const char *expr, char *out, int max_len) {
    out[0] = '\0'; const char *p = expr;
    while (*p) {
        if (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t') { p++; continue; }
        if (*p == '+') { p++; continue; }
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
            char word[64]; int i = 0;
            while (*p && *p != '\"' && *p != '\'' && *p != '+' && *p != ' ' && *p != ')' && *p != ',' && *p != '}' && i < 63)
                word[i++] = *p++;
            word[i] = '\0';
            if (w1_strcmp(word, "null") == 0) { /* skip */ }
            else {
                int rem = max_len - w1_strlen(out) - 1;
                if (rem > 0) w1_strncat(out, word, (size_t)rem);
            }
        }
    }
}

static void eval_attr(warp1_context_t *ctx, warp1_node_t *node, const char *key, char *buf, int size) {
    for (int i = 0; i < node->attrs_count; i++) {
        if (w1_strcmp(node->attrs[i].key, key) == 0) {
            eval_expr(ctx, node->attrs[i].value, buf, size);
            return;
        }
    }
    buf[0] = '\0';
}

// Action executor
static void execute_action1(warp1_context_t *ctx, const char *action_str);
static void execute_script1(warp1_context_t *ctx, const char *name) {
    for (int i = 0; i < ctx->scripts_count; i++) {
        if (w1_strcmp(ctx->scripts[i].name, name) == 0) {
            for (int j = 0; j < ctx->scripts[i].block_count; j++) {
                char cond[256]; w1_strcpy(cond, ctx->scripts[i].blocks[j].condition);
                char *eq = w1_strchr(cond, '=');
                if (eq) {
                    *eq = '\0'; char l_val[512], r_val[512];
                    eval_expr(ctx, cond, l_val, 511);
                    eval_expr(ctx, eq + 1, r_val, 511);
                    if (w1_strcmp(l_val, r_val) == 0) {
                        execute_action1(ctx, ctx->scripts[i].blocks[j].actions);
                        break;
                    }
                } else {
                    execute_action1(ctx, ctx->scripts[i].blocks[j].actions);
                }
            }
            return;
        }
    }
}

static void execute_action1(warp1_context_t *ctx, const char *action_str) {
    if (!action_str || !action_str[0]) return;
    char buf[1024]; w1_strncpy(buf, action_str, 1023); char *p = buf;
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
            char scr[64]; w1_strncpy(scr, act + 10, 63);
            char *end = w1_strchr(scr, '}');
            if (end) { *end = '\0'; }
            set_state(ctx, "_currentScreen", scr);
        } else if (w1_strncmp(act, "script{", 7) == 0) {
            char sname[64]; w1_strncpy(sname, act + 7, 63);
            char *end = w1_strchr(sname, '}');
            if (end) { *end = '\0'; }
            execute_script1(ctx, sname);
        } else if (w1_strchr(act, '.')) {
            char *dot = w1_strchr(act, '.'); *dot = '\0'; char *id = act; char *method = dot + 1;
            char *open_b = w1_strchr(method, '{');
            if (open_b) {
                *open_b = '\0'; char *args = open_b + 1;
                char *close_b = w1_strchr(args, '}');
                if (close_b) { *close_b = '\0'; }
                if (w1_strcmp(method, "changeContent") == 0) {
                    char val[512]; eval_expr(ctx, args, val, 511);
                    char key[128] = "--"; w1_strcat(key, id); w1_strcat(key, "Content");
                    set_state(ctx, key, val);
                }
            }
        } else if (w1_strncmp(act, "--", 2) == 0 || w1_strncmp(act, "~~", 2) == 0) {
            char *eq = w1_strchr(act, '='); if (!eq) eq = w1_strchr(act, ':');
            if (eq) {
                *eq = '\0'; char val[512];
                eval_expr(ctx, eq + 1, val, 511);
                set_state(ctx, act, val);
            }
        }
    }
}

// Tokenizer & Parser
static token1_t next_token(warp1_context_t *ctx) {
    token1_t tk; tk.type = TK1_EOF; tk.val[0] = 0;
    while (*ctx->src_ptr && (unsigned char)*ctx->src_ptr <= 32) ctx->src_ptr++;
    if (!*ctx->src_ptr) return tk;
    if (*ctx->src_ptr == '@') { tk.type = TK1_AT; tk.val[0] = *ctx->src_ptr++; tk.val[1] = 0; return tk; }
    if (*ctx->src_ptr == '\"' || *ctx->src_ptr == '\'') {
        char q = *ctx->src_ptr++; int i = 0; tk.type = TK1_STR;
        while (*ctx->src_ptr && *ctx->src_ptr != q && i < 511) {
            if (*ctx->src_ptr == '\\') {
                tk.val[i++] = *ctx->src_ptr++;
                if (*ctx->src_ptr) tk.val[i++] = *ctx->src_ptr++;
                continue;
            }
            tk.val[i++] = *ctx->src_ptr++;
        }
        if (*ctx->src_ptr == q) { ctx->src_ptr++; }
        tk.val[i] = 0; return tk;
    }
    const char *punct = "{}():;=+,";
    for (int i = 0; punct[i]; i++) {
        if (*ctx->src_ptr == punct[i]) {
            tk.type = TK1_PUNCT; tk.val[0] = *ctx->src_ptr++; tk.val[1] = 0; return tk;
        }
    }
    tk.type = TK1_WORD; int i = 0;
    while (*ctx->src_ptr && (unsigned char)*ctx->src_ptr > 32 && !w1_strchr(punct, *ctx->src_ptr) && i < 511) {
        tk.val[i++] = *ctx->src_ptr++;
    }
    tk.val[i] = 0; return tk;
}

static warp1_node_t *alloc_node(warp1_context_t *ctx) {
    if (ctx->nodes_count < MAX_NODES) {
        warp1_node_t *n = &ctx->nodes[ctx->nodes_count++];
        for (int i = 0; i < (int)sizeof(warp1_node_t); i++) { ((char*)n)[i] = 0; }
        return n;
    }
    return NULL;
}

static warp1_node_t *parse_node(warp1_context_t *ctx);
static void parse_script(warp1_context_t *ctx) {
    ctx->token_pos++; if (ctx->token_pos >= ctx->token_count) return;
    script1_t *s = &ctx->scripts[ctx->scripts_count++]; w1_strncpy(s->name, ctx->tokens[ctx->token_pos].val, 63);
    ctx->token_pos++; if (ctx->tokens[ctx->token_pos].val[0] == '{') {
        ctx->token_pos++;
        while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != '}') {
            if (w1_strcmp(ctx->tokens[ctx->token_pos].val, "if") == 0 || w1_strcmp(ctx->tokens[ctx->token_pos].val, "elseIf") == 0) {
                script_block1_t *b = &s->blocks[s->block_count++]; w1_strcpy(b->type, ctx->tokens[ctx->token_pos].val);
                ctx->token_pos++; if (ctx->tokens[ctx->token_pos].val[0] == ':') ctx->token_pos++;
                if (ctx->tokens[ctx->token_pos].val[0] == '(') {
                    ctx->token_pos++; int p = 1;
                    while (p > 0 && ctx->token_pos < ctx->token_count) {
                        if (ctx->tokens[ctx->token_pos].val[0] == '(') p++;
                        else if (ctx->tokens[ctx->token_pos].val[0] == ')') p--;
                        if (p > 0) { w1_strcat(b->condition, ctx->tokens[ctx->token_pos].val); ctx->token_pos++; }
                    }
                    if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
                }
                if (ctx->tokens[ctx->token_pos].val[0] == '{') {
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
            } else {
                ctx->token_pos++;
            }
        }
        if (ctx->token_pos < ctx->token_count) { ctx->token_pos++; }
    }
}

static warp1_node_t *parse_node(warp1_context_t *ctx) {
    if (ctx->token_pos >= ctx->token_count) return NULL;
    if (ctx->tokens[ctx->token_pos].type == TK1_AT) { parse_script(ctx); return NULL; }
    token1_t t = ctx->tokens[ctx->token_pos];
    if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == '{') {
        warp1_node_t *node = alloc_node(ctx); w1_strcpy(node->tag, t.val); ctx->token_pos += 2;
        while (ctx->token_pos < ctx->token_count && ctx->tokens[ctx->token_pos].val[0] != '}') {
            if (ctx->token_pos + 1 < ctx->token_count && ctx->tokens[ctx->token_pos + 1].val[0] == '{') {
                warp1_node_t *child = parse_node(ctx);
                if (child) node->children[node->children_count++] = child;
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
                } else {
                    w1_strcat(expr, ctx->tokens[ctx->token_pos].val);
                    ctx->token_pos++;
                }
                if (w1_strcmp(key, "oneClick") == 0) {
                    w1_strncpy(node->event_oneclick, expr, 511);
                } else {
                    int idx = node->attrs_count++;
                    w1_strncpy(node->attrs[idx].key, key, 31);
                    w1_strncpy(node->attrs[idx].value, expr, 511);
                }
                if (ctx->tokens[ctx->token_pos].val[0] == ',') {
                    ctx->token_pos++;
                }
                continue;
            }
            ctx->token_pos++;
        }
        if (ctx->token_pos < ctx->token_count) {
            ctx->token_pos++;
        }
        return node;
    }
    ctx->token_pos++; return NULL;
}

// Layout & SVG
static int layout_node1(warp1_context_t *ctx, warp1_node_t *node, int px, int py, int limit_w) {
    if (!node) return 0;
    node->x = px; node->y = py; node->w = limit_w; int cy = py;
    if (w1_strcmp(node->tag, "screen") == 0) {
        for (int i = 0; i < node->children_count; i++) {
            if (w1_strcmp(node->children[i]->tag, "Header") == 0) continue;
            cy += layout_node1(ctx, node->children[i], node->x + 24, cy, limit_w - 48) + 12;
        }
        node->h = cy - py + 24;
    } else if (w1_strcmp(node->tag, "card") == 0) {
        cy += 12; char title[128]; eval_attr(ctx, node, "text", title, 127);
        if (title[0]) {
            ctx->texts[ctx->texts_count].x = px + 24; ctx->texts[ctx->texts_count].y = cy + 4;
            w1_strcpy(ctx->texts[ctx->texts_count].text, title);
            ctx->texts[ctx->texts_count].size = 20;
            ctx->texts[ctx->texts_count].color = 0xFF121212;
            ctx->texts_count++; cy += 36;
        }
        for (int i = 0; i < node->children_count; i++) {
            cy += layout_node1(ctx, node->children[i], px + 24, cy, limit_w - 48) + 8;
        }
        node->h = cy - py + 12;
    } else if (w1_strcmp(node->tag, "button") == 0 || w1_strcmp(node->tag, "tonalButton") == 0) {
        node->h = 40; char text[128]; eval_attr(ctx, node, "text", text, 127);
        node->w = w1_strlen(text) * 10 + 32; if (node->w > limit_w) node->w = limit_w;
        ctx->texts[ctx->texts_count].x = node->x + 16;
        ctx->texts[ctx->texts_count].y = node->y + 10;
        w1_strcpy(ctx->texts[ctx->texts_count].text, text);
        ctx->texts[ctx->texts_count].size = 16;
        ctx->texts[ctx->texts_count].color = 0xFFFFFFFF;
        ctx->texts_count++;
    } else if (w1_strcmp(node->tag, "text") == 0) {
        char text[512]; eval_attr(ctx, node, "text", text, 511);
        ctx->texts[ctx->texts_count].x = px; ctx->texts[ctx->texts_count].y = py;
        w1_strcpy(ctx->texts[ctx->texts_count].text, text);
        ctx->texts[ctx->texts_count].size = 16;
        ctx->texts[ctx->texts_count].color = 0xFF333333;
        ctx->texts_count++;
        int lines = 1; for (int i = 0; text[i]; i++) if (text[i] == '\n') lines++;
        node->h = lines * 22;
    } else if (w1_strcmp(node->tag, "hStack") == 0) {
        int cx = px; int max_h = 0;
        int divisor = node->children_count ? node->children_count : 1;
        for (int i = 0; i < node->children_count; i++) {
            int h = layout_node1(ctx, node->children[i], cx, py, limit_w / divisor);
            if (h > max_h) { max_h = h; }
            cx += node->children[i]->w + 8;
        }
        node->h = max_h;
    } else {
        for (int i = 0; i < node->children_count; i++) {
            cy += layout_node1(ctx, node->children[i], px, cy, limit_w) + 4;
        }
        node->h = cy - py;
    }
    return node->h;
}

static void emit_svg_recursive1(warp1_context_t *ctx, warp1_node_t *node, char *dest, int dest_size) {
    if (!node) return;
    if (w1_strcmp(node->tag, "card") == 0) emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, 32.0f, "#ffffff", "stroke=\"#dddddd\"");
    else if (w1_strcmp(node->tag, "button") == 0) emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, "#0a56d0", "");
    else if (w1_strcmp(node->tag, "tonalButton") == 0) emit_squircle_shape_to(dest, dest_size, node->x, node->y, node->w, node->h, -1.0f, "#0a56d0", "opacity=\"0.1\"");
    for (int i = 0; i < node->children_count; i++) {
        if (w1_strcmp(node->children[i]->tag, "Header") == 0) continue;
        emit_svg_recursive1(ctx, node->children[i], dest, dest_size);
    }
}

// Public API
warp1_context_t* warp1_context_create(const char* code) {
    warp1_context_t* ctx = (warp1_context_t*)malloc(sizeof(warp1_context_t)); if (!ctx) return NULL;
    for (int i = 0; i < (int)sizeof(warp1_context_t); i++) { ((char*)ctx)[i] = 0; }
    ctx->src_ptr = code;
    while (1) {
        token1_t tk = next_token(ctx); if (tk.type == TK1_EOF || ctx->token_count >= MAX_TOKENS) break;
        ctx->tokens[ctx->token_count++] = tk;
    }
    ctx->token_pos = 0;
    while (ctx->token_pos < ctx->token_count) {
        warp1_node_t *node = parse_node(ctx);
        if (node && ctx->root_nodes_count < 16) ctx->root_nodes[ctx->root_nodes_count++] = node;
    }
    warp1_context_update(ctx, 1280, 720); return ctx;
}

void warp1_context_destroy(warp1_context_t* ctx) { if (ctx) free(ctx); }

void warp1_context_update(warp1_context_t* ctx, int width, int height) {
    ctx->texts_count = 0; ctx->svg_output[0] = '\0'; ctx->win_w = width; ctx->win_h = height;
    int total_h = height;
    for (int i = 0; i < ctx->root_nodes_count; i++) {
        int h = layout_node1(ctx, ctx->root_nodes[i], 0, 0, width); if (h > total_h) total_h = h;
    }
    extern char *append_int(char *p, int v); char w_str[16], h_str[16]; append_int(w_str, width); append_int(h_str, total_h);
    w1_strcpy(ctx->svg_output, "<svg width=\""); w1_strcat(ctx->svg_output, w_str); w1_strcat(ctx->svg_output, "\" height=\""); w1_strcat(ctx->svg_output, h_str);
    w1_strcat(ctx->svg_output, "\" xmlns=\"http://www.w3.org/2000/svg\">\n");
    for (int i = 0; i < ctx->root_nodes_count; i++) emit_svg_recursive1(ctx, ctx->root_nodes[i], ctx->svg_output, sizeof(ctx->svg_output));
    w1_strcat(ctx->svg_output, "</svg>");
}

const char* warp1_context_get_svg(warp1_context_t* ctx) { return ctx->svg_output; }
void warp1_context_draw_texts(warp1_context_t* ctx, layer_t* layer, int ox, int oy) {
    for (int i = 0; i < ctx->texts_count; i++) {
        extern void layer_draw_ttf(layer_t *l, int x, int y, const char *s, float sz, uint32_t c);
        layer_draw_ttf(layer, ctx->texts[i].x + ox, ctx->texts[i].y + oy, ctx->texts[i].text, ctx->texts[i].size, ctx->texts[i].color);
    }
}

void warp1_context_click(warp1_context_t* ctx, int x, int y) {
    for (int i = 0; i < ctx->nodes_count; i++) {
        warp1_node_t *n = &ctx->nodes[i];
        if (x >= n->x && x <= n->x + n->w && y >= n->y && y <= n->y + n->h && n->event_oneclick[0]) {
            execute_action1(ctx, n->event_oneclick); break;
        }
    }
    warp1_context_update(ctx, ctx->win_w, ctx->win_h);
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
const char* warp1_context_get_node_svg(warp1_context_t* ctx, int i) { (void)ctx; (void)i; return ""; }
void warp1_context_get_node_prev_rect(warp1_context_t* ctx, int i, int* x, int* y, int* w, int* h) { (void)ctx; (void)i; *x = *y = *w = *h = 0; }
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
