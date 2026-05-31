-- ipc_tool.lua - Moho IPC Tool (播放控制版 v5)
--
-- 功能：
-- 1. IPC Socket 服务 (CFSocket + CFRunLoop)
-- 2. 播放控制 (CFTimer + play/pause/stop/seek)
-- 3. Hook Moho IsPlaying API
-- 4. FFmpeg 视频编码

-- 变量由 wrapper.lua 设置，或使用默认值
IPC_DIR = IPC_DIR or "$IPC_DIR"
USER_PROJECT = USER_PROJECT or "$USER_PROJECT"
USER_SCRIPT = USER_SCRIPT or "$USER_SCRIPT"
IPC_TIMEOUT = IPC_TIMEOUT or tonumber("$IPC_TIMEOUT") or 3600

function MohoScript(moho)
    local log = function(msg)
        print(msg)
        local f = io.open("/tmp/moho_ipc.log", "a")
        if f then f:write(os.date("%H:%M:%S") .. " " .. msg .. "\n") f:close() end
    end

    local f = io.open("/tmp/moho_ipc.log", "w")
    if f then f:close() end

    log("=== IPC 服务启动 ===")

    -- 加载 IPC 模块（统一版：从可执行文件加载）
    local exe_path = IPC_DIR .. "/moho-mate"
    package.cpath = exe_path .. ";" .. package.cpath
    package.loaded["moho_ipc"] = nil

    local ok, ipc = pcall(require, "moho_ipc")
    if not ok then
        log("✗ 模块加载失败: " .. tostring(ipc))
        return
    end
    log("✓ IPC 模块已加载: " .. exe_path)

    -- ===== Helper 管理 =====
    local ipc_helper = nil
    
    -- 获取 moho 对象（优先复用，首次创建）
    local function get_moho()
        if not ipc_helper then
            ipc_helper = MOHO.ScriptInterfaceHelper:new_local()
        end
        return ipc_helper:MohoObject()
    end
    
    -- 释放 helper（退出时调用）
    local function release_helper()
        if ipc_helper then
            ipc_helper:delete()
            ipc_helper = nil
        end
    end

    -- ===== IPC 命令执行 =====
    _G.ipc_execute = function(cmd)
        log("[IPC] 执行: " .. string.sub(cmd, 1, 60))
        
        local moho_obj = get_moho()
        if not moho_obj then
            log("✗ MohoObject 返回 nil")
            release_helper()
            return "error|moho nil"
        end
        
        -- 更新全局引用
        _G.moho = moho_obj
        
        -- 捕获输出
        local output_buffer = {}
        local original_print = print
        print = function(...)
            local args = {}
            for i = 1, select("#", ...) do
                args[i] = tostring(select(i, ...))
            end
            output_buffer[#output_buffer + 1] = table.concat(args, "\t")
            original_print(table.concat(args, "\t"))
        end
        
        -- 执行命令
        local fn, err = load(cmd)
        if not fn then
            print = original_print
            release_helper()
            log("✗ 编译错误: " .. tostring(err))
            return "error|" .. tostring(err)
        end
        
        local ok, result = pcall(fn)
        print = original_print
        -- 不释放 helper，保持 moho 对象可用
        -- release_helper()  -- 注释掉，避免多次命令之间 moho 为 nil
        
        if not ok then
            log("✗ 执行错误: " .. tostring(result))
            return "error|" .. tostring(result)
        end
        
        log("[IPC] ✓ 执行成功")
        local output = table.concat(output_buffer, "\n")
        return "ok|" .. (output == "" and "(无输出)" or output)
    end

    _G.ping = function() return "pong" end

    -- C 代码期望 ipc_dispatch，提供别名
    _G.ipc_dispatch = _G.ipc_execute
    log("✓ ipc_dispatch 已注册")

    _G.ipc_quit = function()
        log("[IPC] quit")
        ipc.stop()
        moho:Quit()
    end

    -- 启动 socket
    log("[IPC] 启动 socket...")
    local running, path = ipc.start()
    if not running then
        log("✗ IPC 启动失败")
        return
    end
    log("✓ Socket: " .. tostring(path))

    -- 创建/打开项目
    if USER_PROJECT and USER_PROJECT ~= "" then
        log("[1] 打开: " .. USER_PROJECT)
        moho:FileOpen(USER_PROJECT)
    else
        log("[1] 新建文档")
        moho:FileNew()
    end

    -- 设置全局 moho 引用
    _G.moho = moho
    log("✓ moho 已更新 (document=" .. (moho.document and "存在" or "nil") .. ")")

    -- 执行用户脚本
    if USER_SCRIPT and USER_SCRIPT ~= "" then
        log("[2] 脚本: " .. USER_SCRIPT)
        local ok, err = pcall(dofile, USER_SCRIPT)
        if not ok then
            log("✗ 脚本错误: " .. tostring(err))
        else
            log("✓ 脚本完成")
        end
    end

    log("=== IPC 运行中 ===")
    log("Socket: /tmp/moho_ipc.sock")
    log("超时: " .. IPC_TIMEOUT .. "s")
end

function LayerScript(moho) end
