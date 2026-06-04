-- test_lua_module.lua
-- 测试 Rust 版 moho-mate 的 Lua 模块

print("=== 测试 moho_ipc Lua 模块 ===")

-- 设置模块路径（moho-mate 可执行文件）
local exe_path = arg[0]:match("(.+)/") or "."
package.cpath = exe_path .. "/moho-mate;" .. package.cpath

print("尝试加载模块...")
print("  路径: " .. exe_path .. "/moho-mate")

-- 加载模块
local ok, ipc = pcall(require, "moho_ipc")
if not ok then
    print("✗ 加载失败: " .. tostring(ipc))
    os.exit(1)
end

print("✓ 模块加载成功")
print("  类型: " .. type(ipc))
print("  函数:")

-- 列出模块函数
for k, v in pairs(ipc) do
    print("    - " .. k .. " (" .. type(v) .. ")")
end

print("\n=== 测试 API ===")

-- 测试 status
local status = ipc.status()
print("status():")
print("  running: " .. tostring(status.running))
print("  path: " .. tostring(status.path))
print("  calls: " .. tostring(status.calls))
print("  errors: " .. tostring(status.errors))

-- 测试 encode_status
local enc = ipc.encode_status()
print("\nencode_status():")
print("  status: " .. tostring(enc.status))
print("  status_text: " .. tostring(enc.status_text))
print("  progress: " .. tostring(enc.progress))

print("\n=== 测试完成 ===")
