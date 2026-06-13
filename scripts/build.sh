#!/bin/bash
# moho-mate 构建脚本
#
# 用法:
#   ./build.sh          # 构建并更新
#   ./build.sh --test   # 构建并测试
#
# 关键步骤:
#   1. 检查 Frameworks 符号链接
#   2. cargo build --release
#   3. 复制到 scripts 目录
#
# ⚠️ 重要：Frameworks 符号链接
#
#   moho-mate 使用 Moho 内置 FFmpeg 库，这些库的 install name 是：
#     @executable_path/../Frameworks/libavcodec.61.dylib
#
#   为了让 moho-mate 能找到这些库，需要在项目根目录创建 Frameworks 符号链接：
#     skills/moho-mate/Frameworks -> /Applications/Moho.app/Contents/Frameworks
#
#   这样从 moho-mate 的视角：
#     @executable_path = scripts/
#     @executable_path/../Frameworks = Frameworks/ -> Moho Frameworks
#
#   好处：
#     - 无需 install_name_tool（零二进制修改）
#     - 无需复制库（省 40MB 空间）
#     - 编译后直接可用
#     - 符号链接永久有效（只需创建一次）
#
# 库位置说明:
#   - macOS:
#       符号链接: skills/moho-mate/Frameworks -> /Applications/Moho.app/Contents/Frameworks/
#       内置库: libavcodec.61.dylib, libavformat.61.dylib, avutil.59.dylib 等
#       scripts 目录: libavfilter.10.dylib（Moho 没有内置）
#   - Windows:
#       所有库: avcodec-61.dll, avformat-61.dll 等
#       位置: Moho 安装目录 或 scripts 目录
#       加载: 通过 PATH 环境变量或 DLL 目录
#       avfilter-10.dll 依赖 avutil-59.dll（需一起分发）
#
# 命名差异:
#   | 平台 | 前缀 | 分隔符 | 后缀 | 示例 |
#   |------|------|--------|------|------|
#   | macOS | lib | . | .dylib | libavfilter.10.dylib |
#   | Windows | 无 | - | .dll | avfilter-10.dll |
#   | Linux | lib | . | .so.X | libavfilter.so.10 |
#
# Windows avfilter-10.dll 获取方式:
#   已通过交叉编译生成（在 macOS 上使用 MinGW-w64）
#   - avfilter-10.dll (22 MB)
#   - avutil-59.dll (3.9 MB)
#   
#   交叉编译命令:
#     ./configure --arch=x86_64 --target-os=mingw64 \
#       --cross-prefix=x86_64-w64-mingw32- \
#       --enable-shared --disable-static \
#       --disable-programs --disable-x86asm \
#       --enable-avfilter
#     make -j8
#
# 相关文件:
#   - build.rs: 设置 rpath (macOS)
#   - encode_native.rs: check_avfilter_available() 检查 scripts 目录
#   - ffmpeg_ffi.rs: FFmpeg FFI 绑定

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
