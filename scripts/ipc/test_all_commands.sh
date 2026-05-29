#!/bin/bash
# moho-mate 所有命令测试脚本

set -e

MOHO_MATE="/Users/def/.openclaw/workspace/skills/moho-mate/scripts/moho-mate"
TEST_PROJECT="/Users/def/.openclaw/workspace/moho_output/Tutorial_1.05_v3.moho"
TEST_OUTPUT="/tmp/moho_test_output"

echo "========================================"
echo "moho-mate 命令测试"
echo "========================================"
echo ""

# 创建测试输出目录
mkdir -p "$TEST_OUTPUT"

# ========== 1. status ==========
echo "=== 测试 status ==="
$MOHO_MATE status
echo ""

# ========== 2. start（无项目）==========
echo "=== 测试 start（无项目）==="
echo "⚠️ 启动 IPC 服务（需要 Moho 已安装）"
pkill -9 Moho 2>/dev/null || true
sleep 2
$MOHO_MATE start --timeout 60
sleep 3
$MOHO_MATE status
echo ""

# ========== 3. call ==========
echo "=== 测试 call（单行命令）==="
$MOHO_MATE call 'print("hello from moho-mate")'
echo ""

# ========== 4. call -f（文件模式）==========
echo "=== 测试 call -f（文件模式）==="
TEST_LUA="/tmp/test_script.lua"
cat > "$TEST_LUA" << 'EOF'
-- 测试脚本
print("✓ 测试脚本开始执行")
print("全局 moho 存在:", moho ~= nil)
if moho then
    print("moho 类型:", type(moho))
    local doc = moho.document
    if doc then
        print("文档名称:", doc:Name())
        print("层数:", doc:CountLayers())
    end
end
print("✓ 测试脚本执行成功")
EOF
$MOHO_MATE call -f "$TEST_LUA"
rm -f "$TEST_LUA"
echo ""

# ========== 5. call（多行命令）==========
echo "=== 测试 call（多行命令）==="
$MOHO_MATE call '
print("✓ 多行命令开始执行")
if moho then
    print("moho 存在")
end
print("✓ 多行命令执行成功")
'
echo ""

# ========== 6. playback status ==========
echo "=== 测试 playback status ==="
$MOHO_MATE playback status
echo ""

# ========== 7. playback play ==========
echo "=== 测试 playback play ==="
$MOHO_MATE playback play 0 24 12
echo ""

# ========== 8. playback pause ==========
echo "=== 测试 playback pause ==="
$MOHO_MATE playback pause
echo ""

# ========== 9. playback stop ==========
echo "=== 测试 playback stop ==="
$MOHO_MATE playback stop
echo ""

# ========== 10. playback seek ==========
echo "=== 测试 playback seek ==="
$MOHO_MATE playback seek 12
echo ""

# ========== 11. render（PNG）==========
echo "=== 测试 render（PNG 序列）==="
if [[ -f "$TEST_PROJECT" ]]; then
    $MOHO_MATE render "$TEST_PROJECT" -f PNG --start 0 --end 3 -o "$TEST_OUTPUT/test_png"
    ls -la "$TEST_OUTPUT"/*.png 2>/dev/null | head -5
    echo "✓ PNG 渲染测试完成"
else
    echo "⚠️ 项目文件不存在: $TEST_PROJECT"
fi
echo ""

# ========== 12. render（GIF）==========
echo "=== 测试 render（GIF）==="
if [[ -f "$TEST_PROJECT" ]]; then
    $MOHO_MATE render "$TEST_PROJECT" -f GIF --start 0 --end 3 -o "$TEST_OUTPUT/test_gif.gif"
    ls -la "$TEST_OUTPUT/test_gif.gif" 2>/dev/null && echo "✓ GIF 渲染测试完成" || echo "✗ GIF 渲染失败"
else
    echo "⚠️ 项目文件不存在"
fi
echo ""

# ========== 13. render（MP4）==========
echo "=== 测试 render（MP4）==="
if [[ -f "$TEST_PROJECT" ]]; then
    $MOHO_MATE render "$TEST_PROJECT" -f MP4 --start 0 --end 3 -o "$TEST_OUTPUT/test.mp4"
    ls -la "$TEST_OUTPUT/test.mp4" 2>/dev/null && echo "✓ MP4 渲染测试完成" || echo "✗ MP4 渲染失败"
else
    echo "⚠️ 项目文件不存在"
fi
echo ""

# ========== 14. encode（PNG → GIF）==========
echo "=== 测试 encode（PNG 序列 → GIF）==="
PNG_DIR="/tmp/moho_test_frames"
mkdir -p "$PNG_DIR"
# 创建测试帧（如果没有真实 PNG）
if [[ -d "$TEST_OUTPUT/test_png" ]] && [[ $(ls "$TEST_OUTPUT/test_png"/*.png 2>/dev/null | wc -l) -gt 0 ]]; then
    cp "$TEST_OUTPUT/test_png"/*.png "$PNG_DIR/"
else
    # 用 ImageMagick 创建测试帧
    for i in {0..3}; do
        convert -size 100x100 xc:blue -pointsize 20 -fill white -gravity center -text 0,0 "Frame $i" "$PNG_DIR/frame_$(printf %04d $i).png" 2>/dev/null || \
        sips -s format png --resampleWidth 100 -o "$PNG_DIR/frame_$(printf %04d $i).png" /System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericApplicationIcon.icns 2>/dev/null || true
    done
fi

if [[ $(ls "$PNG_DIR"/*.png 2>/dev/null | wc -l) -gt 0 ]]; then
    $MOHO_MATE encode "$PNG_DIR/frame_%04d.png" "$TEST_OUTPUT/encoded.gif" --fps 12
    ls -la "$TEST_OUTPUT/encoded.gif" 2>/dev/null && echo "✓ GIF 编码测试完成" || echo "✗ GIF 编码失败"
else
    echo "⚠️ 无测试帧，跳过"
fi
rm -rf "$PNG_DIR"
echo ""

# ========== 15. start + script ==========
echo "=== 测试 start + script ===="
pkill -9 Moho 2>/dev/null || true
sleep 2
TEST_START_SCRIPT="/tmp/test_start_script.lua"
cat > "$TEST_START_SCRIPT" << 'EOF'
print("✓ 启动脚本执行")
if moho then
    print("moho 可用")
end
EOF
$MOHO_MATE start "$TEST_PROJECT" "$TEST_START_SCRIPT" --timeout 60
sleep 3
$MOHO_MATE status
rm -f "$TEST_START_SCRIPT"
echo ""

# ========== 16. draw ==========
echo "=== 测试 draw ===="
$MOHO_MATE draw circle
echo ""
$MOHO_MATE draw bunny
echo ""
$MOHO_MATE draw puppy
echo ""
echo "⚠️ draw 只绘制，不保存（IPC 限制）"
echo "✓ draw 测试完成"
echo ""

# ========== 17. inspect ==========
echo "=== 测试 inspect ===="
if [[ -f "$TEST_PROJECT" ]]; then
    $MOHO_MATE inspect "$TEST_PROJECT"
    echo "✓ inspect 测试完成"
else
    echo "⚠️ 项目文件不存在: $TEST_PROJECT"
fi
echo ""

# ========== 18. config ==========
echo "=== 测试 config ===="
$MOHO_MATE config list
echo ""
echo "备份配置..."
$MOHO_MATE config backup
echo "✓ config backup 完成"
echo ""
echo "恢复配置..."
$MOHO_MATE config restore
echo "✓ config restore 完成"
echo ""

# ========== 19. render（halfsize）==========
echo "=== 测试 render（halfsize）==="
if [[ -f "$TEST_PROJECT" ]]; then
    $MOHO_MATE render "$TEST_PROJECT" -f PNG --start 0 --end 3 -halfsize yes -o "$TEST_OUTPUT/test_halfsize"
    ls -la "$TEST_OUTPUT"/test_halfsize/*.png 2>/dev/null | head -5
    echo "✓ halfsize 渲染测试完成"
else
    echo "⚠️ 项目文件不存在: $TEST_PROJECT"
fi
echo ""

# ========== 20. encode（PNG → MP4）==========
echo "=== 测试 encode（PNG 序列 → MP4）==="
PNG_DIR="/tmp/moho_test_frames"
mkdir -p "$PNG_DIR"
# 使用之前渲染的 PNG 或创建测试帧
if [[ -d "$TEST_OUTPUT/test_png" ]] && [[ $(ls "$TEST_OUTPUT/test_png"/*.png 2>/dev/null | wc -l) -gt 0 ]]; then
    cp "$TEST_OUTPUT/test_png"/*.png "$PNG_DIR/"
fi

if [[ $(ls "$PNG_DIR"/*.png 2>/dev/null | wc -l) -gt 0 ]]; then
    $MOHO_MATE encode "$PNG_DIR/frame_%04d.png" "$TEST_OUTPUT/encoded.mp4" --fps 24 --crf 18
    ls -la "$TEST_OUTPUT/encoded.mp4" 2>/dev/null && echo "✓ MP4 编码测试完成" || echo "✗ MP4 编码失败"
else
    echo "⚠️ 无测试帧，跳过"
fi
rm -rf "$PNG_DIR"
echo ""

# ========== 16. quit ==========
echo "=== 测试 quit ==="
$MOHO_MATE quit
sleep 2
$MOHO_MATE status
echo ""

# ========== 清理 ==========
echo "=== 清理测试输出 ==="
rm -rf "$TEST_OUTPUT"
rm -f /tmp/test_script.lua
echo "✓ 清理完成"
echo ""

echo "========================================"
echo "测试完成"
echo "========================================"
