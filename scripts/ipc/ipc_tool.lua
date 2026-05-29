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

    -- 加载 IPC 模块
    local ipc_so = IPC_DIR .. "/moho_ipc.so"
    package.cpath = ipc_so .. ";" .. package.cpath
    package.loaded["moho_ipc"] = nil

    local ok, ipc = pcall(require, "moho_ipc")
    if not ok then
        log("✗ 模块加载失败: " .. tostring(ipc))
        return
    end
    log("✓ IPC 模块已加载")

    -- ===== 检查 Moho 原生播放状态 =====
    -- 不 Hook IsPlaying，直接提供独立函数
    _G.ipc_is_moho_playing = function()
        -- 方案1：尝试调用 moho:IsPlaying()
        local ok, result = pcall(function()
            return moho:IsPlaying()
        end)
        if ok then
            return result == true
        end
        -- 方案2：检查 moho.document 的播放状态
        local doc = moho.document
        if doc then
            -- Moho 文档可能有播放帧范围设置
            -- 暂时返回 false，依赖 IPC playback 状态
        end
        return false
    end
    
    _G.ipc_can_start_playback = function()
        -- 检查 Moho 原生是否在播放
        local moho_playing = ipc_is_moho_playing()
        if moho_playing then
            return false, "Moho 正在播放，请先停止"
        end
        -- 检查 IPC 是否已在播放
        if ipc.is_playing() then
            return false, "IPC playback 已在运行"
        end
        return true, "ok"
    end

    -- ===== Helper 管理 =====
    local ipc_helper = nil
    
    -- 获取 moho 对象（优先复用，首次创建）
    local function get_moho()
        if not ipc_helper then
            ipc_helper = MOHO.ScriptInterfaceHelper:new_local()
        end
        return ipc_helper:MohoObject()
    end
    
    -- 释放 helper（仅在播放结束时调用）
    local function release_helper()
        if ipc_helper then
            ipc_helper:delete()
            ipc_helper = nil
        end
    end
    
    -- ===== 播放帧回调 =====
    -- 方案 3：tick 机制 - 由 Moho Idle 回调触发
    _G.ipc_play_frame = function(frame)
        local moho_obj = get_moho()
        if moho_obj then
            moho_obj:SetCurFrame(frame, false, false)  -- updateUI=false, boneDynamics=false
        end
    end
    
    -- ===== Idle 回调（Moho 每帧渲染时调用）=====
    _G.ipc_idle = function()
        local ipc = require('moho_ipc')
        if ipc.is_playing() then
            ipc.tick()  -- 触发帧更新
        end
    end
    
    -- ===== 播放事件回调 =====
    _G.ipc_play_event = function(event)
        -- 静默处理，减少日志输出
        if event == "complete" or event == "stop" then
            release_helper()
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
        _G.MOHO = MOHO
        _G.LM = LM
        
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

    -- 保存 moho 引用
    _G._ipc_moho = moho
    _G.moho = moho
    log("✓ moho 已更新 (document=" .. (moho.document and "存在" or "nil") .. ")")

    -- 执行用户脚本
    if USER_SCRIPT and USER_SCRIPT ~= "" then
        log("[2] 脚本: " .. USER_SCRIPT)
        _G.moho = moho
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
    log("")
    log("播放命令 (使用 moho-mate CLI):")
    log("  moho-mate playback play 0 72 24   -- 播放")
    log("  moho-mate playback pause          -- 暂停/恢复")
    log("  moho-mate playback stop           -- 停止")
    log("  moho-mate playback seek 36        -- 跳转")
    log("  moho-mate playback status         -- 查看状态")
end

function LayerScript(moho) end
