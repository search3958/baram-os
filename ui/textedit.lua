-- textedit.lua
-- テキストエディタの Lua スクリプト

-- 行数管理
local line_count = 1

-- 初期化処理
warp1_set_state("--status", "Ready")
warp1_set_state("--current_file", "(新規ファイル)")
warp1_set_state("--char_count", "文字数：0")
warp1_set_state("--line_count", "行数：1")
warp1_set_state("--input_1", "")
