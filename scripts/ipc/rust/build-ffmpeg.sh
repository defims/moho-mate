#!/bin/bash
# 编译 moho-mate 并自动修复 dylib 链接

set -e

cd "$(dirname "$0")"

MOHO_FW="/Applications/Moho.app/Contents/Frameworks"
SCRIPTS_DIR="/Users/def/.openclaw/workspace/skills/moho-mate/scripts"
PKG_CONFIG_PATH="$SCRIPTS_DIR/pkgconfig"
TARGET="target/release/moho-mate"

echo "▶ 编译 moho-mate (ffmpeg-builtin)..."
export FFMPEG_PKG_CONFIG_PATH="$PKG_CONFIG_PATH"
export PKG_CONFIG_PATH="$PKG_CONFIG_PATH"

PATH="$HOME/.cargo/bin:$PATH" cargo build --release --features ffmpeg-builtin

if [ ! -f "$TARGET" ]; then
    echo "✗ 编译失败"
    exit 1
fi

echo "▶ 修复 dylib 链接..."

# libavfilter 来自 scripts
install_name_tool -change \
    "@loader_path/libavfilter.10.dylib" \
    "$SCRIPTS_DIR/libavfilter.10.dylib" \
    "$TARGET"

# 其他 FFmpeg 库来自 Moho
for lib in avcodec.61 avformat.61 avutil.59 swscale.8 swresample.5; do
    install_name_tool -change \
        "@executable_path/../Frameworks/lib$lib.dylib" \
        "$MOHO_FW/lib$lib.dylib" \
        "$TARGET"
done

echo "✓ 完成: $TARGET"
echo ""
echo "验证:"
otool -L "$TARGET" | grep -E "avcodec|avformat|avutil|swscale|swresample|avfilter"