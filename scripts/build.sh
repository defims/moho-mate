#!/bin/bash
# moho-mate 构建脚本
#
# 用法:
#   ./build.sh          # 构建并更新
#   ./build.sh --test   # 构建并测试
#
# ⚠️ 关键：scripts 目录库符号链接方案
#
# ## 最终方案
#
# 在 scripts 目录创建所有库的符号链接：
# ```text
# skills/moho-mate/scripts/
# ├── moho-mate
# ├── libavfilter.10.dylib
# ├── libavcodec.61.dylib -> /Applications/Moho.app/.../libavcodec.61.dylib
# ├── libavformat.61.dylib -> /Applications/Moho.app/.../libavformat.61.dylib
# ├── libavutil.59.dylib -> ...
# ├── libswscale.8.dylib -> ...
# └── libswresample.5.dylib -> ...
# ```
#
# ## 为什么可行？
#
# 1. 修改 moho-mate 的库引用路径：
#    @executable_path/../Frameworks/ → @executable_path/
#
#    当 moho-mate 运行时（在 scripts/ 目录）：
#    @executable_path = scripts/
#    @executable_path/libavcodec.61.dylib = scripts/libavcodec.61.dylib ✅
#
# 2. 库之间的依赖自动解决：
#    libavformat 依赖：@loader_path/../Frameworks/libavcodec.61.dylib
#
#    关键：@loader_path 解析为**真实文件所在目录**，不是符号链接所在目录
#
#    当 libavformat.61.dylib 符号链接指向 Moho Frameworks：
#    @loader_path = /Applications/Moho.app/Contents/Frameworks/
#    @loader_path/../Frameworks = /Applications/Moho.app/.../Frameworks/ ✅
#
# ## 对比两种方案
#
# | 项目 | 方案 A（Frameworks 符号链接） | 方案 B（scripts 库符号链接） |
# |------|------------------------------|----------------------------|
# | 符号链接位置 | 项目根目录 | scripts 目录 |
# | 符号链接数量 | 1 个 | 5 个 |
# | install_name_tool | 不需要 | 需要 |
# | 库集中 | 分散 | scripts 目录 |
# | 目录结构 | Frameworks/ + scripts/ | 只有 scripts/ |
#
# 方案 B 更清晰：所有文件集中在 scripts 目录。
#
# ## 相关文件
#
# - build.rs: 设置 rpath
# - encode_native.rs: FFmpeg 编码实现
# - ffmpeg_ffi.rs: FFmpeg FFI 绑定

set -e
cd "$(dirname "$0")/moho-mate-src"

echo "=== 构建 moho-mate ==="
cargo build --release

echo ""
echo "=== 更新 moho-mate ==="
cp target/release/moho-mate ../moho-mate

if [ "$(uname)" = "Darwin" ]; then
    # ============================================================
    # macOS: 在 scripts 目录创建库符号链接
    # ============================================================
    
    echo ""
    echo "=== 创建库符号链接 ==="
    cd ..
    
    MOHO_FRAMEWORKS="/Applications/Moho.app/Contents/Frameworks"
    LIBS="libavcodec.61.dylib libavformat.61.dylib libavutil.59.dylib libswscale.8.dylib libswresample.5.dylib"
    
    for lib in $LIBS; do
        if [ ! -L "$lib" ]; then
            ln -sf "$MOHO_FRAMEWORKS/$lib" "$lib"
            echo "✓ 创建符号链接: $lib -> $MOHO_FRAMEWORKS/$lib"
        fi
    done
    
    # ============================================================
    # 修改 moho-mate 的库引用路径
    # ============================================================
    
    echo ""
    echo "=== 修改库引用路径 ==="
    
    for lib in $LIBS; do
        install_name_tool -change "@executable_path/../Frameworks/$lib" \
                                 "@executable_path/$lib" \
                                 moho-mate
    done
    
    echo "✓ 库引用路径已修改"
fi

if [ "$1" = "--test" ]; then
    echo ""
    echo "=== 测试 IPC 启动 ==="
    pkill -9 Moho 2>/dev/null || true
    rm -f /tmp/moho_ipc.sock /tmp/moho_ipc.log /tmp/moho_wrapper.lua
    rm -rf /tmp/moho_ipc_config_backup
    sleep 2
    ../moho-mate start --timeout 30
fi