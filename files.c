#include "fs.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

// Warp1 エンジンのヘッダをインクルード
#include "ui/warp1_engine.h"

// ファイルマネージャーのメイン初期化関数
void init_file_manager(warp1_context_t *ctx) {
    if (!ctx) return;

    // 1. ファイルシステムからファイル一覧を取得
    uint32_t count = g_sb.num_files;
    char status_msg[128];
    sprintf(status_msg, "Total: %d files found in initrd.", (int)count);
    
    // ステータステキストを更新
    warp1_context_set_state(ctx, "fs_status/text", status_msg);

    // 2. ファイルごとに動的に UI 要素を追加
    for (uint32_t i = 0; i < count; i++) {
        const char *name = g_sb.entries[i].name;
        
        // ._ で始まる隠しファイル（macOS由来等）はスキップ
        if (name[0] == '.' && name[1] == '_') continue;

        char row_id[32], btn_h_id[32], exec_id[32], del_id[32];
        sprintf(row_id, "file_row_%d", (int)i);
        sprintf(btn_h_id, "btn_h_%d", (int)i);
        sprintf(exec_id, "exec_%d", (int)i);
        sprintf(del_id, "del_%d", (int)i);

        // --- 行コンテナ (hStack) ---
        warp1_context_add_node(ctx, "file_list", "hStack", row_id);
        warp1_context_set_attr(ctx, row_id, "padding", "10");
        warp1_context_set_attr(ctx, row_id, "backgroundColor", "#2a2a2a");
        warp1_context_set_attr(ctx, row_id, "spacing", "10");

        // --- ファイル名テキスト ---
        char text_id[32];
        sprintf(text_id, "text_%d", (int)i);
        warp1_context_add_node(ctx, row_id, "text", text_id);
        warp1_context_set_attr(ctx, text_id, "text", name);
        warp1_context_set_attr(ctx, text_id, "color", "#ffffff");
        warp1_context_set_attr(ctx, text_id, "size", "16");

        // --- ボタン用コンテナ (hStack) ---
        warp1_context_add_node(ctx, row_id, "hStack", btn_h_id);
        warp1_context_set_attr(ctx, btn_h_id, "spacing", "5");

        // --- 実行ボタン (Execute) ---
        // .warp または .bin の場合に表示
        if (strstr(name, ".warp") || strstr(name, ".warpc") || strstr(name, ".bin")) {
            warp1_context_add_node(ctx, btn_h_id, "button", exec_id);
            warp1_context_set_attr(ctx, exec_id, "text", "Run");
            warp1_context_set_attr(ctx, exec_id, "backgroundColor", "#27ae60");
            
            // onClick アクションを設定（os_open_file 等のシステムコール想定）
            char cmd[128];
            sprintf(cmd, "run{os_open_file:\"%s\"}", name);
            warp1_context_set_attr(ctx, exec_id, "oneClick", cmd);
        }

        // --- 削除ボタン (Delete) ---
        warp1_context_add_node(ctx, btn_h_id, "button", del_id);
        warp1_context_set_attr(ctx, del_id, "text", "Del");
        warp1_context_set_attr(ctx, del_id, "backgroundColor", "#e74c3c");
        warp1_context_set_attr(ctx, del_id, "oneClick", "run{os_show_log:\"Delete not implemented\"}");
    }
}
