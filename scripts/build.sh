#!/bin/bash
# moho-mate 构建脚本
#
# 用法:
#   ./build.sh          # 构建并更新
#   ./build.sh --test   # 构建并测试
#
# ⚠️ 关键：Frameworks 符号链接方案
#
# 这是替代 install_name_tool 的更优雅方案。
#
# ## 背景
#
# Moho 内置 FFmpeg 库的 install name：
#   @executable_path/../Frameworks/libavcodec.61.dylib
#
# 当 moho-mate 运行时（在 scripts/ 目录）：
#   @executable_path = scripts/
#   @executable_path/../Frameworks = skills/moho-mate/Frameworks/
#
# ## 解决方案
#
# 在项目根目录创建 Frameworks 符号链接：
#   skills/moho-mate/Frameworks -> /Applications/Moho.app/Contents/Frameworks
#
# ## 为什么有效？
#
# 1. 路径自动解析：
#    @executable_path/../Frameworks/libavcodec.61.dylib
#    → skills/moho-mate/Frameworks/libavcodec.61.dylib
#    → /Applications/Moho.app/Contents/Frameworks/libavcodec.61.dylib ✅
#
# 2. libavfilter 使用 @rpath：
#    @rpath/libavfilter.10.dylib
#    rpath 在 build.rs 中设置为 scripts/
#    → scripts/libavfilter.10.dylib ✅
#
# ## 为什么不在 scripts/ 目录创建库符号链接？
#
# 因为库之间也有依赖：
#   libavformat.61.dylib
#     └── @loader_path/../Frameworks/libavcodec.61.dylib
#
#   @loader_path = 当前库所在目录
#
# 如果符号链接在 scripts/：
#   @loader_path/../Frameworks = scripts/../Frameworks = skills/moho-mate/Frameworks
#
# 最终还是需要 Frameworks 符号链接，所以在 scripts/ 目录创建库符号链接无效。
#
# ## 对比
#
# | 方案 | 符号链接数量 | install_name_tool | 用户干预 |
# |------|-------------|-------------------|--------|
# | ~~旧方案~~ | 0 | 需要（每次编译） | 无 |
# | **新方案** | 1（Frameworks） | 不需要 | 一次 |
#
# 新方案更优：
# - 符号链接永久有效
# - 无需修改二进制
# - 编译后直接可用
#
# ## 库位置
#
# macOS:
#   符号链接: skills/moho-mate/Frameworks -> /Applications/Moho.app/Contents/Frameworks/
#   内置库: libavcodec.61.dylib, libavformat.61.dylib, libavutil.59.dylib,
#           libswscale.8.dylib, libswresample.5.dylib
#   scripts: libavfilter.10.dylib（Moho 没有内置）
#
# Windows:
#   所有库: avcodec-61.dll, avformat-61.dll 等
#   位置: Moho 安装目录 或 scripts 目录
#   加载: 通过 PATH 环境变量
#   avfilter-10.dll 依赖 avutil-59.dll（需一起分发）
#
# ## 命名差异
#
# | 平台 | 前缀 | 分隔符 | 后缀 | 示例 |
# |------|------|--------|------|------|
# | macOS | lib | . | .dylib | libavfilter.10.dylib |
# | Windows | 无 | - | .dll | avfilter-10.dll |
# | Linux | lib | . | .so.X | libavfilter.so.10 |
#
# ## 相关文件
#
# - build.rs: 设置 rpath
# - encode_native.rs: FFmpeg 编码实现
# - ffmpeg_ffi.rs: FFmpeg FFI 绑定

set -e
cd "$(dirname "$0")/moho-mate-src"

# ============================================================
# macOS: 检查并创建 Frameworks 符号链接
# ============================================================
# 
# 这是替代 install_name_tool 的更优雅方案。
# 符号链接让 @executable_path/../Frameworks 自动指向 Moho Frameworks。
#
if [ "$(uname)" = "Darwin" ]; then
    # 注意：build.sh 在 scripts/moho-mate-src/ 中执行
    # .. = scripts/，所以需要 ../.. 才能到达项目根目录
    PROJECT_ROOT="$(cd ../.. && pwd)"
    FRAMEWORKS_LINK="$PROJECT_ROOT/Frameworks"
    MOHO_FRAMEWORKS="/Applications/Moho.app/Contents/Frameworks"
    
    # 检查 Moho 是否安装
    if [ ! -d "$MOHO_FRAMEWORKS" ]; then
        echo "⚠️ Moho Frameworks 目录不存在: $MOHO_FRAMEWORKS"
        echo "   请确保 Moho 已安装"
        exit 1
    fi
    
    # 检查符号链接是否存在
    if [ ! -L "$FRAMEWORKS_LINK" ]; then
        echo "=== 创建 Frameworks 符号链接 ==="
        ln -s "$MOHO_FRAMEWORKS" "$FRAMEWORKS_LINK"
        echo "✓ 符号链接已创建: $FRAMEWORKS_LINK -> $MOHO_FRAMEWORKS"
    else
        # 验证符号链接目标是否正确
        LINK_TARGET="$(readlink "$FRAMEWORKS_LINK")"
        if [ "$LINK_TARGET" != "$MOHO_FRAMEWORKS" ]; then
            echo "⚠️ 符号链接目标不正确: $LINK_TARGET"
            echo "   重新创建正确的符号链接"
            rm "$FRAMEWORKS_LINK"
            ln -s "$MOHO_FRAMEWORKS" "$FRAMEWORKS_LINK"
            echo "✓ 符号链接已更新: $FRAMEWORKS_LINK -> $MOHO_FRAMEWORKS"
        else
            echo "✓ Frameworks 符号链接已存在"
        fi
    fi
fi

echo ""
echo "=== 构建 moho-mate ==="
cargo build --release

# 注意：不再需要 install_name_tool！
# 符号链接已经让 @executable_path/../Frameworks 指向 Moho Frameworks

echo ""
echo "=== 更新 moho-mate ==="
cp target/release/moho-mate ../moho-mate
echo "✓ 已更新: $(pwd)/../moho-mate"

if [ "$1" = "--test" ]; then
    echo ""
    echo "=== 测试 IPC 启动 ==="
    pkill -9 Moho 2>/dev/null || true
    rm -f /tmp/moho_ipc.sock /tmp/moho_ipc.log /tmp/moho_wrapper.lua
    rm -rf /tmp/moho_ipc_config_backup
    sleep 2
    ../moho-mate start --timeout 30
fi
