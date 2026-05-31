-- ipc_tool.lua - Moho IPC Tool
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

-- ===== Moho Helper =====
local ipc_helper = nil

local function get_moho()
    if not ipc_helper then
        ipc_helper = MOHO.ScriptInterfaceHelper:new_local()
    end
    return ipc_helper:MohoObject()
end

local function release_helper()
    if ipc_helper then
        ipc_helper:delete()
        ipc_helper = nil
    end
end

-- ===== IPC 命令执行 =====
local original_print = print
local output_buffer = {}

local function capture_print(...)
    local args = {}
    for i = 1, select("#", ...) do
        args[i] = tostring(select(i, ...))
    end
    output_buffer[#output_buffer + 1] = table.concat(args, "\t")
    original_print(table.concat(args, "\t"))
end

_G.ipc_execute = function(cmd)
    log("[IPC] 执行: " .. string.sub(cmd, 1, 60))

    local moho_obj = get_moho()
    if not moho_obj then
        log("✗ MohoObject 返回 nil")
        return "error|moho nil"
    end

    _G.moho = moho_obj

    -- 捕获输出
    output_buffer = {}
    print = capture_print

    local fn, err = load(cmd)
    if not fn then
        print = original_print
        log("✗ 编译错误: " .. tostring(err))
        return "error|" .. tostring(err)
    end

    local ok, result = pcall(fn)
    print = original_print

    if not ok then
        log("✗ 执行错误: " .. tostring(result))
        return "error|" .. tostring(result)
    end

    log("[IPC] ✓ 执行成功")
    local output = table.concat(output_buffer, "\n")
    return "ok|" .. (output == "" and "(无输出)" or output)
end

_G.ipc_quit = function()
    log("[IPC] quit")
    ipc.stop()
    release_helper()
    moho:Quit()
end

-- ===== 主入口 =====
function MohoScript(moho)
    log_clear()
    log("=== IPC v" .. VERSION .. " ===")

    -- 加载 IPC 模块
    local exe_path = IPC_DIR .. "/moho-mate"
    package.cpath = exe_path .. ";" .. package.cpath
    package.loaded["moho_ipc"] = nil

    local ok, ipc = pcall(require, "moho_ipc")
    if not ok then
        log("✗ 模块加载失败: " .. tostring(ipc))
        return
    end
    log("✓ 模块已加载: " .. exe_path)

    -- 启动 socket
    local running, path = ipc.start()
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
