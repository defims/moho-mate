#!/bin/bash
# moho-mate 构建脚本
#
# 用法:
#   ./build.sh          # 构建并更新
#   ./build.sh --test   # 构建并测试
#
# 关键步骤:
#   1. cargo build --release
#   2. install_name_tool 修改库路径 (macOS only)
#   3. 复制到 scripts 目录
#
# 平台差异:
#   - macOS: 需要 install_name_tool 修改库路径
#   - Windows: 无需修改，依赖 PATH 或 DLL 目录
#   - Linux: 无需修改，依赖 LD_LIBRARY_PATH
#
# 为什么需要 install_name_tool (macOS)?
#   - Moho 内置的 FFmpeg 库使用 @executable_path/../Frameworks/ 路径
#   - moho-mate 不在 Moho.app 目录，运行时找不到库
#   - install_name_tool 将路径改为绝对路径，解决运行时加载问题
#
# 库路径说明:
#   - macOS:
#       Moho 内置库（libavcodec, libavformat 等）:
#         原路径: @executable_path/../Frameworks/libavcodec.61.dylib
#         新路径: /Applications/Moho.app/Contents/Frameworks/libavcodec.61.dylib
#       libavfilter:
#         路径: @rpath/libavfilter.10.dylib
#         rpath: scripts 目录（在 build.rs 中设置）
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

echo "=== 构建 moho-mate ==="
cargo build --release

echo ""
echo "=== 修改 FFmpeg 库路径 ==="
MOHO_MATE="target/release/moho-mate"

# 修改 Moho 内置库的路径为绝对路径
install_name_tool -change "@executable_path/../Frameworks/libavcodec.61.dylib" "/Applications/Moho.app/Contents/Frameworks/libavcodec.61.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libavformat.61.dylib" "/Applications/Moho.app/Contents/Frameworks/libavformat.61.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libavutil.59.dylib" "/Applications/Moho.app/Contents/Frameworks/libavutil.59.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libswscale.8.dylib" "/Applications/Moho.app/Contents/Frameworks/libswscale.8.dylib" "$MOHO_MATE" 2>/dev/null || true
install_name_tool -change "@executable_path/../Frameworks/libswresample.5.dylib" "/Applications/Moho.app/Contents/Frameworks/libswresample.5.dylib" "$MOHO_MATE" 2>/dev/null || true

echo "✓ FFmpeg 库路径已修改"

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
