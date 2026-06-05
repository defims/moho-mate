#!/bin/bash
# Download FFmpeg 7.0 headers for moho-mate skill
# Usage: ./download-ffmpeg-headers.sh

set -e

SKILL_DIR="$HOME/.openclaw/workspace/skills/moho-mate"
INCLUDE_DIR="$SKILL_DIR/include"
TMP_DIR="/tmp/ffmpeg-7.0-headers-$$"

echo "Downloading FFmpeg 7.0 headers..."

# Try different methods
if command -v git &> /dev/null; then
    echo "Method 1: Git clone..."
    if git clone --depth 1 --branch n7.0 https://github.com/FFmpeg/FFmpeg.git "$TMP_DIR" 2>/dev/null; then
        echo "Git clone succeeded!"
    else
        echo "Git clone failed, trying tarball..."
        rm -rf "$TMP_DIR"
    fi
fi

if [ ! -d "$TMP_DIR" ]; then
    echo "Method 2: Download tarball..."
    TARBALL="/tmp/ffmpeg-7.0.tar.xz"
    if curl -L -o "$TARBALL" "https://ffmpeg.org/releases/ffmpeg-7.0.tar.xz" 2>/dev/null; then
        echo "Tarball downloaded, extracting..."
        tar -xf "$TARBALL" -C /tmp
        TMP_DIR="/tmp/ffmpeg-7.0"
        rm -f "$TARBALL"
    fi
fi

if [ ! -d "$TMP_DIR" ]; then
    echo "ERROR: Failed to download FFmpeg 7.0"
    echo "Please manually download from: https://ffmpeg.org/releases/ffmpeg-7.0.tar.xz"
    echo "Extract and copy headers to: $INCLUDE_DIR"
    exit 1
fi

# Copy headers
echo "Copying headers..."
for lib in libavcodec libavformat libavutil libswscale libswresample libavfilter; do
    mkdir -p "$INCLUDE_DIR/$lib"
    cp -r "$TMP_DIR/$lib"/*.h "$INCLUDE_DIR/$lib/" 2>/dev/null || true
done

# Cleanup
rm -rf "$TMP_DIR"

echo ""
echo "✅ FFmpeg 7.0 headers installed to:"
echo "   $INCLUDE_DIR"
echo ""
echo "To use with ffmpeg-next, set:"
echo "   export PKG_CONFIG_PATH=\"$SKILL_DIR/pkgconfig:\$PKG_CONFIG_PATH\""
