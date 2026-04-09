-- files.lua
-- ファイルマネージャー Lua 版

local function init_file_manager()
    log("File Manager: Starting...")
    
    -- サンプルファイルリスト
    local files = {
        "main.warpc",
        "terminal.warp", 
        "textedit.warp",
        "files.warp",
        "test.lua",
        "os_settings.json"
    }
    
    for i, name in ipairs(files) do
        if string.sub(name, 1, 2) ~= "._" then
            local row_id = "file_row_" .. i
            local text_id = "text_" .. i
            local btn_h_id = "btn_h_" .. i
            
            -- 行コンテナ (hStack)
            warp.addNode("file_list", "hStack", row_id)
            warp.setAttr(row_id, "padding", "10")
            warp.setAttr(row_id, "backgroundColor", "#2a2a2a")
            warp.setAttr(row_id, "spacing", "10")
            
            -- ファイル名テキスト
            warp.addNode(row_id, "text", text_id)
            warp.setAttr(text_id, "text", name)
            warp.setAttr(text_id, "color", "#ffffff")
            warp.setAttr(text_id, "size", "16")
            
            -- ボタン用コンテナ (hStack)
            warp.addNode(row_id, "hStack", btn_h_id)
            warp.setAttr(btn_h_id, "spacing", "5")
            
            -- 実行ボタン（.warp, .warpc の場合）
            if string.find(name, ".warp") or string.find(name, ".warpc") then
                local exec_id = "exec_" .. i
                warp.addNode(btn_h_id, "button", exec_id)
                warp.setAttr(exec_id, "text", "Run")
                warp.setAttr(exec_id, "backgroundColor", "#27ae60")
                warp.setAttr(exec_id, "oneClick", 'run{os_open_file:"' .. name .. '"}')
            end
            
            -- 削除ボタン
            local del_id = "del_" .. i
            warp.addNode(btn_h_id, "button", del_id)
            warp.setAttr(del_id, "text", "Del")
            warp.setAttr(del_id, "backgroundColor", "#e74c3c")
            warp.setAttr(del_id, "oneClick", 'run{os_delete_file:"' .. name .. '"}')
        end
    end
    
    log("File Manager: " .. #files .. " files displayed")
end

-- 初期化実行
init_file_manager()
