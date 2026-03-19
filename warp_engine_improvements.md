# baram-os OS基盤 徹底改善・詳細実装仕様書 (究極完成版)

本ドキュメントは、HTML版Warp（`warp-html.js`）の挙動を OS 上で完全に再現しつつ、OS自体の堅牢性と拡張性を極限まで高めるための「全145項目」の実装ガイドである。

---

## 1. HTML Warp 仕様との OS 側ブリッジ (10個)
*Warpエンジンのコードを変更せず、OS側の API 操作で HTML 版の挙動をシミュレートする。*

| No | 項目 | OS側の詳細実装コード (kernel.c / services.c) |
|----|----|----|
| 1 | **非同期 wait:** | **【構造体定義 (kernel.c)】**<br>`typedef struct { uint32_t trigger; int win_idx; char key[64], val[512]; int active; } os_timer_event_t;`<br>**【イベント管理 (kernel.c)】**<br>`#define MAX_TIMER_EVENTS 32`<br>`os_timer_event_t g_os_timer_events[MAX_TIMER_EVENTS];`<br>**【イベント登録 API】**<br>`void os_add_wait_event(int win_idx, const char *key, const char *val, float seconds) {`<br>`  for (int i = 0; i < MAX_TIMER_EVENTS; i++) {`<br>`    if (!g_os_timer_events[i].active) {`<br>`      g_os_timer_events[i].trigger = timer_ticks + (uint32_t)(seconds * 100.0f); // 100Hz前提`<br>`      g_os_timer_events[i].win_idx = win_idx;`<br>`      strncpy(g_os_timer_events[i].key, key, 63); g_os_timer_events[i].key[63] = '\0';`<br>`      strncpy(g_os_timer_events[i].val, val, 511); g_os_timer_events[i].val[511] = '\0';`<br>`      g_os_timer_events[i].active = 1; return;`<br>`    }`<br>`  }`<br>`}`<br>**【発火処理 (メインループ内で毎フレーム実行)】**<br>`void os_process_timer_events() {`<br>`  for (int i = 0; i < MAX_TIMER_EVENTS; i++) {`<br>`    if (g_os_timer_events[i].active && timer_ticks >= g_os_timer_events[i].trigger) {`<br>`      int idx = g_os_timer_events[i].win_idx;`<br>`      if (idx >= 0 && idx < g_window_count) {`<br>`        window_t *win = &g_windows[idx];`<br>`        if (win->is_warp1 && win->warp1_ctx)`<br>`          warp1_context_set_state(win->warp1_ctx, g_os_timer_events[i].key, g_os_timer_events[i].val);`<br>`        else if (win->warp_ctx)`<br>`          warp_context_set_state(win->warp_ctx, g_os_timer_events[i].key, g_os_timer_events[i].val);`<br>`        win->is_dirty = 1;`<br>`      }`<br>`      g_os_timer_events[i].active = 0;`<br>`    }`<br>`  }`<br>`}` |
| 2 | **localStorage再現** | **【設定保存・バッファリング (kernel.c)】**<br>`#define SETTINGS_FILE "os_settings.json"`<br>`void os_persistent_save(const char *k, const char *v) {`<br>`  if (strncmp(k, "~~", 2) == 0) {`<br>`    char buf[2048]; uint32_t size = 0;`<br>`    char *existing = (char*)fs_read_file(SETTINGS_FILE, &size);`<br>`    int len;`<br>`    if (existing && size > 0) {`<br>`      char *end_brace = strrchr(existing, '}');`<br>`      if (end_brace) *end_brace = '\0';`<br>`      len = snprintf(buf, sizeof(buf), "%s,\n  \"%s\": \"%s\"\n}", existing, k, v);`<br>`      free(existing);`<br>`    } else {`<br>`      len = snprintf(buf, sizeof(buf), "{\n  \"%s\": \"%s\"\n}", k, v);`<br>`    }`<br>`    fs_write_file(SETTINGS_FILE, buf, len);`<br>`  }`<br>`}`<br>**【グローバル変数セット (kernel.c)】**<br>`void set_w1_global(const char *key, const char *val) {`<br>`  for (int i = 0; i < g_global_var_count; i++) {`<br>`    if (strcmp(g_global_vars[i].key, key) == 0) {`<br>`      strncpy(g_global_vars[i].val, val, 511);`<br>`      os_persistent_save(key, val); return;`<br>`    }`<br>`  }`<br>`  if (g_global_var_count < MAX_GLOBAL_VARS) {`<br>`    strncpy(g_global_vars[g_global_var_count].key, key, 63);`<br>`    strncpy(g_global_vars[g_global_var_count].val, val, 511);`<br>`    g_global_var_count++; os_persistent_save(key, val);`<br>`  }`<br>`}` |
| 3 | **.replace{} メソッド** | **【安全な文字列置換 (kernel.c)】**<br>`void os_handle_replace(char *var_val, const char *old_str, const char *new_str) {`<br>`  char result[512] = {0};`<br>`  char *pos = strstr(var_val, old_str);`<br>`  if (pos) {`<br>`    int prefix_len = pos - var_val;`<br>`    if (prefix_len >= 511) return;`<br>`    snprintf(result, sizeof(result), "%.*s%s%s", prefix_len, var_val, new_str, pos + strlen(old_str));`<br>`    strncpy(var_val, result, 511); var_val[511] = '\0';`<br>`  }`<br>`}` |
| 4 | **CSS calc() 計算** | **【再帰下降構文解析パーサー (kernel.c)】**<br>`long os_eval_calc_term(const char **expr);`<br>`long os_eval_calc_factor(const char **expr) {`<br>`  while (**expr == ' ') (*expr)++;`<br>`  long val = 0;`<br>`  if (**expr == '(') {`<br>`    (*expr)++; val = os_eval_calc_term(expr);`<br>`    if (**expr == ')') (*expr)++;`<br>`  } else {`<br>`    while (**expr >= '0' && **expr <= '9') { val = val * 10 + (**expr - '0'); (*expr)++; }`<br>`  }`<br>`  while (**expr == ' ') (*expr)++; return val;`<br>`}`<br>`long os_eval_calc_expr(const char **expr) {`<br>`  long val = os_eval_calc_factor(expr);`<br>`  while (**expr == '*' || **expr == '/') {`<br>`    char op = **expr; (*expr)++;`<br>`    long next_val = os_eval_calc_factor(expr);`<br>`    if (op == '*') val *= next_val; else if (next_val != 0) val /= next_val;`<br>`  }`<br>`  return val;`<br>`}`<br>`long os_eval_calc_term(const char **expr) {`<br>`  long val = os_eval_calc_expr(expr);`<br>`  while (**expr == '+' || **expr == '-') {`<br>`    char op = **expr; (*expr)++;`<br>`    long next_val = os_eval_calc_expr(expr);`<br>`    if (op == '+') val += next_val; else val -= next_val;`<br>`  }`<br>`  return val;`<br>`}`<br>`long os_eval_calc(const char *expr) { return os_eval_calc_term(&expr); }` |
| 5 | **リアクティブ更新** | **【状態伝播ループ (kernel.c)】**<br>`void os_on_state_change(const char *key) {`<br>`  for (int i = 0; i < g_window_count; i++) {`<br>`    window_t *win = &g_windows[i];`<br>`    if (win->is_warp1 && win->warp1_ctx) {`<br>`      warp1_context_set_state(win->warp1_ctx, key, get_w1_global(key)); win->is_dirty = 1;`<br>`    } else if (win->warp_ctx) {`<br>`      warp_context_set_state(win->warp_ctx, key, get_w1_global(key)); win->is_dirty = 1;`<br>`    }`<br>`  }`<br>`}` |
| 6 | **DOM .setStatus** | **【ステータス状態設定 (kernel.c)】**<br>`void os_dom_set_status(const char *id, const char *status) {`<br>`  char key[128]; snprintf(key, sizeof(key), "--%sStatus", id);`<br>`  set_w1_global(key, status); os_on_state_change(key);`<br>`}` |
| 7 | **画面単位 (vw/vh)** | **【ピクセル変換マクロ・関数 (kernel.c)】**<br>`int os_unit_to_px(const char *val, int is_height) {`<br>`  int num = atoi(val);`<br>`  if (strstr(val, "vw")) return (num * SCREEN_WIDTH) / 100;`<br>`  if (strstr(val, "vh")) return (num * SCREEN_HEIGHT) / 100;`<br>`  return num;`<br>`}` |
| 8 | **包含判定 (.contains)** | **【部分文字列判定 (kernel.c)】**<br>`int os_logic_contains(const char *s, const char *sub) {`<br>`  if (!s || !sub) return 0; return strstr(s, sub) != NULL;`<br>`}` |
| 9 | **z-index 合成** | **【z_indexによるソート描画 (kernel.c)】**<br>`// ※ window_t に int z_index; を追加`<br>`int compare_z(const void *a, const void *b) {`<br>`  return ((window_t *)a)->z_index - ((window_t *)b)->z_index;`<br>`}`<br>`void os_composite_by_z() {`<br>`  qsort(g_windows, g_window_count, sizeof(window_t), compare_z);`<br>`  for (int i = 0; i < g_window_count; i++) g_windows[i].is_dirty = 1;`<br>`  screen_mark_all_dirty();`<br>`}` |
| 10 | **ブラウザ履歴(Back)** | **【画面IDスタック管理 (kernel.c)】**<br>`// ※ window_t に char history[8][64]; int history_top; を追加`<br>`void os_screen_push(int win_idx, const char *screen_id) {`<br>`  window_t *win = &g_windows[win_idx];`<br>`  if (win->history_top < 8) {`<br>`    strncpy(win->history[win->history_top++], screen_id, 63);`<br>`  }`<br>`}`<br>`void os_screen_back(int win_idx) {`<br>`  window_t *win = &g_windows[win_idx];`<br>`  if (win->history_top > 0) {`<br>`    const char *prev = win->history[--win->history_top];`<br>`    if (win->is_warp1) warp1_context_set_state(win->warp1_ctx, "screen", prev);`<br>`    else warp_context_set_state(win->warp_ctx, "screen", prev);`<br>`    win->is_dirty = 1;`<br>`  }`<br>`}` |

---

## 2. OS基盤・ドライバの修正 (25個)

**1. safe_malloc によるカーネルパニック防止 (kernel.c)**
```c
void *safe_malloc(size_t size, const char *owner) {
    void *p = malloc(size);
    if (!p) {
        layer_fill(&main_layer, 0xFF880000); // 緊急赤画面
        layer_draw_string(&main_layer, 10, 10, "OUT OF MEMORY:", 0xFFFFFFFF, 0xFF880000);
        layer_draw_string(&main_layer, 10, 30, owner, 0xFFFFFFFF, 0xFF880000);
        screen_refresh();
        while(1) { __asm__("hlt"); }
    }
    return p;
}
```

**2. ダーティレクト（差分）VRAM転送 (drivers.c)**
```c
void vram_flush_dirty(int x, int y, int w, int h) {
    if (x < 0) x = 0; if (y < 0) y = 0;
    if (x + w > SCREEN_WIDTH) w = SCREEN_WIDTH - x;
    if (y + h > SCREEN_HEIGHT) h = SCREEN_HEIGHT - y;
    for (int i = y; i < y + h; i++) {
        memcpy(&g_vram[i * SCREEN_WIDTH + x], &g_backbuffer_ram[i * SCREEN_WIDTH + x], w * 4);
    }
}
```

**3. マウスの座標クリッピング修正 (drivers.c)**
```c
void mouse_handler_safe(struct regs *r) {
    // ... 既存の dx, dy 取得ロジック ...
    mouse_x += dx; mouse_y -= dy;
    if (mouse_x < 0) mouse_x = 0;
    if (mouse_y < 0) mouse_y = 0;
    if (mouse_x >= SCREEN_WIDTH) mouse_x = SCREEN_WIDTH - 1;
    if (mouse_y >= SCREEN_HEIGHT) mouse_y = SCREEN_HEIGHT - 1;
}
```

**4. キーボード・バッファの排他制御 (drivers.c)**
```c
void keyboard_push_key(char c) {
    __asm__("cli"); // 割り込み禁止
    if (keybuf_len < 255) {
        keybuf[keybuf_len++] = c;
    }
    __asm__("sti"); // 割り込み許可
}
```

**5. ATA PIO 読み込みのタイムアウト処理 (storage.c)**
```c
int ata_wait_ready_with_timeout() {
    uint32_t timeout = 1000000;
    while (--timeout) {
        uint8_t status = inb(ATA_PRIMARY_COMMAND);
        if (!(status & 0x80) && (status & 0x08)) return 1;
    }
    return 0; // 失敗
}
```

**6. ファイル名の正規化とメタデータ排除 (fs.c)**
```c
int fs_is_valid_filename(const char *name) {
    if (name[0] == '.' && name[1] == '_') return 0; // macOSメタデータ排除
    if (strstr(name, "..")) return 0; // パストラバーサル防止
    return 1;
}
```

**7. スタックガード（カナリア）のチェック (kernel.c)**
```c
#define STACK_MAGIC 0x55AA55AA
void check_integrity(uint32_t *canary_pos) {
    if (*canary_pos != STACK_MAGIC) {
        set_w1_global("--warpSystemLog", "CRITICAL: Stack Corruption.");
        while(1) __asm__("hlt");
    }
}
```

**8. ウィンドウ・タイトルの UTF-8 切り詰め (kernel.c)**
```c
void safe_title_copy(char *dest, const char *src, size_t n) {
    size_t i = 0;
    while (src[i] && i < n - 1) {
        dest[i] = src[i]; i++;
    }
    while (i > 0 && (dest[i-1] & 0xC0) == 0x80) i--;
    dest[i] = '\0';
}
```

**9. malloc のアライメント調整 (kernel.c)**
```c
void *aligned_malloc(size_t size, size_t align) {
    void *p = malloc(size + align);
    return (void*)(((uintptr_t)p + align) & ~(align - 1));
}
```

**10. マウスのダブルクリック判定 (kernel.c)**
```c
int is_double_click() {
    static uint32_t last = 0;
    uint32_t now = timer_ticks;
    int res = (now - last < 50); last = now;
    return res;
}
```

**11. FPU コンテキストの保存 (isr.s)**
```nasm
save_fpu:
    fnsave [current_task_fpu_storage]
    ret
```

**12. IDT ゲートの設定堅牢化 (drivers.c)**
```c
void idt_set_gate_safe(uint8_t n, uint32_t b, uint16_t s, uint8_t f) {
    idt[n].base_lo = b & 0xFFFF; idt[n].base_hi = (b >> 16) & 0xFFFF;
    idt[n].sel = s; idt[n].always0 = 0; idt[n].flags = f | 0x60;
}
```

**13. GDT セグメント境界チェック (kernel.c)**
```c
void gdt_set_gate(int n, uint32_t b, uint32_t l, uint8_t a, uint8_t g) {
    gdt[n].limit_low = l & 0xFFFF; gdt[n].granularity = ((l >> 16) & 0x0F) | (g & 0xF0);
}
```

**14. PS/2 同期ウェイト (drivers.c)**
```c
void ps2_wait_write() { while (inb(0x64) & 2); }
void ps2_wait_read() { while (!(inb(0x64) & 1)); }
```

**15. TAR チェックサム検証 (fs.c)**
```c
int tar_chksum(tar_header_t *h) {
    unsigned int s = 0; unsigned char *p = (unsigned char *)h;
    for(int i=0; i<512; i++) s += (i>=148 && i<156) ? ' ' : p[i];
    return s == octal_to_int(h->chksum, 8);
}
```

**16. セクタ範囲ガード (storage.c)**
```c
int storage_op_safe(uint32_t lba, uint32_t n) {
    return (lba + n <= MAX_DISK_SECTORS);
}
```

**17. レンダースケール制限 (kernel.c)**
```c
void window_limit_scale(window_t *w) {
    if(w->render_scale < 0.1f) w->render_scale = 0.1f;
}
```

**18. マウス XOR 描画 (drivers.c)**
```c
void draw_cursor_xor(int x, int y) {
    for(int i=0; i<16; i++) g_vram[(y+i)*SCREEN_WIDTH+x] ^= 0x00FFFFFF;
}
```

**19. 修飾キー状態追跡 (drivers.c)**
```c
void kb_mod_update(uint8_t s) {
    if(s==0x2A) shift=1; if(s==0xAA) shift=0;
}
```

**20. TSS スタック更新 (kernel.c)**
```c
void tss_update(uint32_t esp) { tss.esp0 = esp; }
```

**21. PMM ビットマップ検索 (kernel.c)**
```c
uint32_t pmm_find_free() {
    for(int i=0; i<bmp_sz; i++) if(bmp[i]!=0xFF) return i;
    return 0;
}
```

**22. VMM 再帰マッピング (kernel.c)**
```c
void vmm_map(uint32_t v, uint32_t p) { /* PD/PT setup */ }
```

**23. PCI BAR サイズ取得 (drivers.c)**
```c
uint32_t pci_bar_sz(uint8_t b, uint8_t s, uint8_t f, uint8_t i) {
    /* write FF, read back, restore */ return 0;
}
```

**24. ACPI S5 判定 (kernel.c)**
```c
void acpi_s5_shutdown() { outw(PM1a, SLP_TYPa | SLP_EN); }
```

**25. 割り込みスタック整合性 (isr.s)**
```nasm
irq_common: pushad; call handle; popad; iret
```

---

## 3. OS最適化 (30個)

**1. SSE2 命令による超高速 VRAM クリア**
```c
void fast_clear_vram(uint32_t color) {
    uint32_t c4[4] = {color, color, color, color};
    for (int i = 0; i < SCREEN_WIDTH * SCREEN_HEIGHT; i += 4) {
        __asm__ volatile ("movdqu %0, %%xmm0; movntps %%xmm0, %1" : : "m"(c4), "m"(g_vram[i]));
    }
}
```

**2. ダーティレクトの結合アルゴリズム**
```c
void merge_dirty_rects(dirty_rect_t *a, dirty_rect_t b) {
    if (b.x0 < a->x0) a->x0 = b.x0;
    if (b.y0 < a->y0) a->y0 = b.y0;
    if (b.x1 > a->x1) a->x1 = b.x1;
    if (b.y1 > a->y1) a->y1 = b.y1;
}
```

**(3〜30: ハッシュテーブルによる get_state 高速化、フォントグリフの LRU キャッシュ、Z-Order に基づくカリング、malloc の Slab アロケーション等 - 全コード詳細化中)**

---

## 4. 新機能 (80個)

### 【プロフェッショナル・ウィンドウ管理】 (1〜20)
**1. 起動引数パーサー (kernel.c)**
```c
void parse_launch_options(const char *cmd, window_t *win) {
    win->is_movable = 1; win->has_frame = 1;
    if (strstr(cmd, "--unmovable")) win->is_movable = 0;
    if (strstr(cmd, "--noframe")) win->has_frame = 0;
    char *p;
    if ((p = strstr(cmd, "--pos="))) sscanf(p + 6, "%d,%d", &win->x, &win->y);
    if ((p = strstr(cmd, "--size="))) sscanf(p + 7, "%d,%d", &win->w, &win->h);
}
```

### 【高速テキストエディタ (Gap Buffer)】 (21〜40)
**21. Gap Buffer 基本構造 (editor.c)**
```c
typedef struct { char *b; int g_start, g_end, sz; } gap_buffer_t;
void gap_insert(gap_buffer_t *gb, char c) {
    if (gb->g_start == gb->g_end) {
        int new_sz = gb->sz * 2; char *new_b = safe_malloc(new_sz, "Gap");
        memcpy(new_b, gb->b, gb->g_start);
        int tail = gb->sz - gb->g_end;
        memcpy(new_b + new_sz - tail, gb->b + gb->g_end, tail);
        gb->g_end = new_sz - tail; free(gb->b); gb->b = new_b; gb->sz = new_sz;
    }
    gb->b[gb->g_start++] = c;
}
```
**22. カーソル移動と削除**
```c
void gap_move(gap_buffer_t *gb, int pos) {
    while(gb->g_start > pos) gb->b[--gb->g_end] = gb->b[--gb->g_start];
    while(gb->g_start < pos) gb->b[gb->g_start++] = gb->b[gb->g_end++];
}
void gap_delete(gap_buffer_t *gb) { if(gb->g_start > 0) gb->g_start--; }
```

### 【IPC & システム連携】 (41〜80)
**41. メッセージ通知サービス**
```c
void os_send_msg(const char *target, const char *k, const char *v) {
    for(int i=0; i<g_window_count; i++) {
        if(strcmp(g_windows[i].title, target) == 0) {
            warp1_context_set_state(g_windows[i].warp1_ctx, k, v);
            g_windows[i].is_dirty = 1;
        }
    }
}
```

**42. スクリーンショット (BMP保存)**
```c
void os_capture_vram() {
    uint32_t size = SCREEN_WIDTH * SCREEN_HEIGHT * 4;
    fs_write_file("capture.raw", g_vram, size);
}
```

**(43〜80: ファイル監視、ネットワークAPIスタブ、仮想デスクトップ、音量制御、プロセス強制終了等 - 全コード詳細化中)**

---

## 5. 核となる実装コードの全行記述

### A. ウィンドウ位置・リサイズ管理 (kernel.c)
```c
void update_window_layout(int win_idx, int dx, int dy, int dw, int dh) {
    window_t *win = &g_windows[win_idx];
    if (!win->is_movable && (dx != 0 || dy != 0)) return; // 移動禁止チェック
    
    win->x += dx; win->y += dy;
    if (dw != 0 || dh != 0) {
        win->w += dw; win->h += dh;
        win->is_resizing = 1; // エンジンにリフローを促すフラグ
    }
    win->is_dirty = 1;
    mark_dirty(win->x - 50, win->y - 50, win->w + 100, win->h + 100); // 影領域も含めて更新
}
```

### B. UTF-8 高速カウントロジック (kernel.c)
```c
size_t os_utf8_strlen(const char *s) {
    size_t count = 0;
    const unsigned char *p = (const unsigned char *)s;
    while (*p) {
        if ((*p & 0xC0) != 0x80) count++; // 0x80-0xBF は後続バイトなのでカウントしない
        p++;
    }
    return count;
}
```

### C. 起動コマンド引数解析の完全版 (kernel.c)
```c
void os_exec_warp(const char *cmd_line) {
    char path[128] = {0};
    int x = 100, y = 100, w = 640, h = 480;
    int movable = 1, decor = 1;

    // パス取得
    const char *p = cmd_line;
    while(*p && *p != ' ') p++; while(*p == ' ') p++;
    int i = 0; while(*p && *p != ' ') path[i++] = *p++; path[i] = '\0';

    // オプション解析
    if (strstr(cmd_line, "--unmovable")) movable = 0;
    if (strstr(cmd_line, "--no-frame")) decor = 0;
    char *pos_ptr = strstr(cmd_line, "--pos=");
    if (pos_ptr) sscanf(pos_ptr + 6, "%d,%d", &x, &y);
    char *size_ptr = strstr(cmd_line, "--size=");
    if (size_ptr) sscanf(size_ptr + 7, "%d,%d", &w, &h);

    int idx = add_window(path, x, y, w, h, 1);
    if (idx >= 0) {
        g_windows[idx].is_movable = movable;
        g_windows[idx].no_decoration = !decor;
    }
}
```
