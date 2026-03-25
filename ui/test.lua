log("Hello from Lua!")
warp.setState("--luaStatus", "Active")
warp.addNode("main", "text", "lua_msg")
warp.setAttr("lua_msg", "text", "This message was added by Lua!")
