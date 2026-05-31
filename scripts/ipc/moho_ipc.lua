-- moho_ipc.lua - Moho IPC 初始化脚本
-- 版本: 1.0.0
-- 功能: IPC Socket 服务 + FFmpeg 视频编码

local VERSION = "1.0.0"

-- 变量由 moho-mate start 命令设置
IPC_DIR = IPC_DIR or "$IPC_DIR"
USER_PROJECT = USER_PROJECT or "$USER_PROJECT"
USER_SCRIPT = USER_SCRIPT or "$USER_SCRIPT"
IPC_TIMEOUT = IPC_TIMEOUT or tonumber("$IPC_TIMEOUT") or 3600

-- ===== 日志 =====
local LOG_FILE = "/tmp/moho_ipc.log"

local function log(msg)
    print(msg)
    local f = io.open(LOG_FILE, "a")
    if f then
        f:write(os.date("%H:%M:%S") .. " " .. msg .. "\n")
        f:close()
    end
end

local function log_clear()
    local f = io.open(LOG_FILE, "w")
    if f then f:close() end
end

-- ===== IPC 命令执行 (C 实现) =====
-- execute_via_helper 在 moho_ipc.c 中直接实现
-- moho_ipc.quit() 在 moho_ipc.c 中直接实现

-- ===== 主入口 =====
function MohoScript(moho)
    log_clear()
    log("=== IPC v" .. VERSION .. " ===")

    -- 加载 IPC 模块
    local exe_path = IPC_DIR .. "/moho-mate"
    package.cpath = exe_path .. ";" .. package.cpath
    package.loaded["moho_ipc"] = nil

    local ok, ipc_module = pcall(require, "moho_ipc")
    if not ok then
        log("✗ 模块加载失败: " .. tostring(ipc_module))
        return
    end
    _G.moho_ipc = ipc_module  -- 设置全局 moho_ipc
    log("✓ 模块已加载: " .. exe_path)

    -- 启动 socket
    local running, path = moho_ipc.start()
    if not running then
        log("✗ IPC 启动失败")
        return
    end
    log("✓ Socket: " .. tostring(path))

    -- 创建/打开项目
    if USER_PROJECT and USER_PROJECT ~= "" then
        log("[项目] 打开: " .. USER_PROJECT)
        moho:FileOpen(USER_PROJECT)
    else
        log("[项目] 新建文档")
        moho:FileNew()
    end

    _G.moho = moho
    log("✓ moho 已就绪")

    -- 执行用户脚本
    if USER_SCRIPT and USER_SCRIPT ~= "" then
        log("[脚本] 执行: " .. USER_SCRIPT)
        local ok, err = pcall(dofile, USER_SCRIPT)
        if not ok then
            log("✗ 脚本错误: " .. tostring(err))
        else
            log("✓ 脚本完成")
        end
    end

    log("=== IPC 运行中 (超时: " .. IPC_TIMEOUT .. "s) ===")
end
