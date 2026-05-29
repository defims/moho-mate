-- test_encode.lua - 测试 moho_ipc.so encode GIF 功能
-- 运行: lua test_encode.lua

local IPC_DIR = "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc"

-- 加载模块
package.cpath = IPC_DIR .. "/moho_ipc.so;" .. package.cpath
local ok, ipc = pcall(require, "moho_ipc")
if not ok then
    print("✗ 加载失败: " .. tostring(ipc))
    os.exit(1)
end
print("✓ 模块已加载")

-- 检查函数
print("\n=== 模块函数 ===")
for k, v in pairs(ipc) do
    print("  " .. k .. " = " .. type(v))
end

-- 测试 encode_status
print("\n=== 测试 encode_status ===")
local status = ipc.encode_status()
for k, v in pairs(status) do
    print("  " .. k .. " = " .. tostring(v))
end

-- 测试 encode GIF
print("\n=== 测试 encode GIF ===")
local input_pattern = "/tmp/test_encode_frames/frame_%04d.png"
local output_path = "/tmp/test_encode_output.gif"

print("输入: " .. input_pattern)
print("输出: " .. output_path)

local success, msg = ipc.encode(input_pattern, output_path, {fps = 10, crf = 23})
print("结果: " .. tostring(success) .. ", " .. tostring(msg))

if success then
    -- 等待编码完成
    print("\n=== 等待编码 ===")
    local max_wait = 30
    local wait_count = 0
    
    while wait_count < max_wait do
        os.execute("sleep 1")
        wait_count = wait_count + 1
        status = ipc.encode_status()
        print(string.format("%d: status=%s progress=%.1f%% frames=%d", 
            wait_count, status.status_text, status.progress * 100, status.encoded_frames or 0))
        
        if status.status == 2 then
            print("✓ 编码成功!")
            break
        elseif status.status == 3 then
            print("✗ 编码失败: " .. tostring(status.error_msg))
            break
        end
    end
    
    -- 检查输出文件
    print("\n=== 检查输出 ===")
    if os.execute("test -f " .. output_path) then
        os.execute("ls -la " .. output_path)
        print("✓ GIF 文件已生成")
    else
        print("✗ 输出文件不存在")
    end
end

print("\n=== 测试完成 ===")