-- moho_ipc.lua - Moho IPC 初始化脚本
-- 版本: 2.0.0 (Rust 统一版)
-- 功能: IPC Socket 服务 + FFmpeg 视频编码

local VERSION = "2.0.0"

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

-- ===== 主入口 =====
function MohoScript(moho)
    log_clear()
    log("=== IPC v" .. VERSION .. " (Rust 统一版) ===")

    -- ⚠️ 验证启动令牌（只有 moho-mate 创建的 wrapper.lua 才能启动）
    local token = IPC_START_TOKEN or ""
    if token == "" or token == "$IPC_START_TOKEN" then
        log("✗ 启动拒绝：缺少启动令牌")
        log("⚠️ 只有 moho-mate 创建的 wrapper.lua 才能启动 IPC")
        return
    end

    -- 读取令牌文件验证
    local token_file = io.open("/tmp/moho_ipc_token", "r")
    if not token_file then
        log("✗ 启动拒绝：令牌文件不存在")
        return
    end

    local expected_token = token_file:read("*l")
    token_file:close()

    if IPC_START_TOKEN ~= expected_token then
        log("✗ 启动拒绝：令牌验证失败")
        log("  期望: " .. tostring(expected_token))
        log("  收到: " .. tostring(IPC_START_TOKEN))
        return
    end

    log("✓ 启动令牌验证通过")

    -- 加载 IPC 模块（从 moho-mate 可执行文件）
    -- Rust 版本：可执行文件静态链接 Lua，可被 dlopen 加载
    local exe_path = IPC_DIR .. "/moho-mate"

    -- ⚠️ macOS: 可执行文件可以被 dlopen 加载（需要 -Wl,-export_dynamic）
    package.cpath = exe_path .. ";" .. package.cpath
    package.loaded["moho_ipc"] = nil

    local ok, ipc_module = pcall(require, "moho_ipc")
    if not ok then
        log("✗ 模块加载失败: " .. tostring(ipc_module))
        log("  尝试路径: " .. exe_path)
        return
    end

    -- ⚠️ 立即从 package.loaded 删除（防止其他脚本 require）
    package.loaded["moho_ipc"] = nil
    log("✓ 模块已加载并隔离（package.loaded 已清除）")

    -- 存到 registry（给 C 的 execute_via_helper 用）
    local registry = debug.getregistry()
    registry._ipc_module = ipc_module
    log("✓ 模块已存到 registry（隔离保护）")

    -- 启动 socket（用局部变量）
    local running, path = ipc_module.start()
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
