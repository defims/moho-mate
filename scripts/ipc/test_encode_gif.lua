-- test_encode_gif.lua
-- 通过 IPC 测试 encode GIF 功能

local input_pattern = "/tmp/test_encode_frames/frame_%04d.png"
local output_path = "/tmp/test_encode_output.gif"

print("=== 测试 encode GIF ===")
print("输入: " .. input_pattern)
print("输出: " .. output_path)

-- 加载 IPC 模块
local ipc_dir = "/Users/def/.openclaw/workspace/skills/moho-mate/scripts/ipc"
package.cpath = ipc_dir .. "/moho_ipc.so;" .. package.cpath

local ok, ipc = pcall(require, "moho_ipc")
if not ok then
    print("✗ 加载 moho_ipc 失败: " .. tostring(ipc))
    return
end

print("✓ moho_ipc 已加载")

-- 测试 encode
local success, msg = ipc.encode(input_pattern, output_path, {fps = 10})
print("encode 返回: " .. tostring(success) .. ", " .. tostring(msg))

if success then
    print("等待编码完成...")
    for i = 1, 30 do
        os.execute("sleep 1")
        local status = ipc.encode_status()
        print(string.format("[%d] status=%d (%s) progress=%.1f frames=%d",
            i, status.status, status.status_text, status.progress * 100, status.encoded_frames or 0))
        
        if status.status == 2 then
            print("✓ 编码成功!")
            break
        elseif status.status == 3 then
            print("✗ 编码失败: " .. tostring(status.error_msg))
            break
        end
    end
end

print("=== 测试完成 ===")
ipc_quit()
