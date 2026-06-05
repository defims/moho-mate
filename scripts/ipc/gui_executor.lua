-- gui_executor.lua
-- 方案A: Main Thread GUI 命令执行器
-- 在 Moho 中作为定时器运行，轮询并执行 GUI 命令

local M = {}

-- 检查是否有待处理的 GUI 命令
function M.poll()
    local ipc = debug.getregistry()._ipc_module
    if not ipc then
        return false
    end
    
    -- 检查是否有待处理命令
    if not ipc.has_gui_pending() then
        return false
    end
    
    -- 获取命令
    local cmd_id, cmd = ipc.get_pending_gui_command()
    if not cmd_id then
        return false
    end
    
    print(string.format("[GUI Executor] 执行命令 #%d: %s", cmd_id, cmd))
    
    -- 执行命令
    local success, result = pcall(function()
        local fn, err = load(cmd)
        if not fn then
            return false, "load error: " .. tostring(err)
        end
        
        local ok, res = pcall(fn)
        return ok, tostring(res or "")
    end)
    
    -- 设置结果
    if success then
        ipc.set_gui_result(cmd_id, true, result or "")
        print(string.format("[GUI Executor] 命令 #%d 完成", cmd_id))
    else
        ipc.set_gui_result(cmd_id, false, tostring(result))
        print(string.format("[GUI Executor] 命令 #%d 失败: %s", cmd_id, tostring(result)))
    end
    
    return true
end

-- 启动定时器轮询
function M.start_poller(moho, interval_ms)
    interval_ms = interval_ms or 100  -- 默认 100ms
    
    -- 使用 Moho 的定时器功能
    local timer = moho.document:CreateTimer("GUIExecutor", interval_ms, function()
        M.poll()
    end)
    
    print("[GUI Executor] 定时器已启动，间隔 " .. interval_ms .. "ms")
    return timer
end

return M
