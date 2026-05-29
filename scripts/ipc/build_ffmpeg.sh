#!/bin/bash
# 编译 moho_ipc.so（静态链接 Lua + FFmpeg）

set -e

LUA_SRC="/tmp/lua-5.4.4/src"
FFMPEG_SRC="/tmp/ffmpeg-7.0"
MOHO_FRAMEWORKS="/Applications/Moho.app/Contents/Frameworks"
IPC_DIR="$HOME/.openclaw/workspace/skills/moho-mate/scripts/ipc"
OUTPUT="$IPC_DIR/moho_ipc.so"

cd /tmp

# 检查 Lua 源码
if [[ ! -d "$LUA_SRC" ]]; then
    echo "下载 Lua 5.4.4..."
    curl -sL https://www.lua.org/ftp/lua-5.4.4.tar.gz | tar xz
fi

# 检查 FFmpeg 头文件
if [[ ! -d "$FFMPEG_SRC" ]]; then
    echo "❌ 请先下载 FFmpeg 7.0 头文件:"
    echo "   cd /tmp && git clone -c http.proxy='http://127.0.0.1:7897' --depth 1 --branch n7.0 https://github.com/FFmpeg/FFmpeg.git ffmpeg-7.0-headers"
    exit 1
fi

# 编译 Lua 核心文件
LUA_FILES=(
    lapi.c lcode.c lctype.c ldebug.c ldump.c ldo.c lfunc.c lgc.c
    llex.c lmem.c lobject.c lopcodes.c lparser.c lstate.c lstring.c
    ltable.c ltm.c lundump.c lvm.c lzio.c lauxlib.c linit.c
    lbaselib.c lcorolib.c ldblib.c liolib.c lmathlib.c loadlib.c
    loslib.c lstrlib.c ltablib.c lutf8lib.c
)

echo "=== 编译 Lua 5.4.4 核心 ==="
OBJS=""
for f in "${LUA_FILES[@]}"; do
    echo "编译: $f"
    gcc -c -O2 -DLUA_COMPAT_5_3 -I"$LUA_SRC" -o "${f%.c}.o" "$LUA_SRC/$f"
    OBJS="$OBJS ${f%.c}.o"
done

echo "=== 编译 moho_ipc.c（链接 FFmpeg）==="
gcc -c -O2 \
    -I"$LUA_SRC" \
    -I"$FFMPEG_SRC" \
    -o moho_ipc.o "$IPC_DIR/moho_ipc.c"

echo "=== 链接 moho_ipc.so ==="
# 动态链接 Moho 内置的 FFmpeg 库
gcc -bundle -flat_namespace -undefined suppress \
    -o "$OUTPUT" moho_ipc.o $OBJS \
    -framework CoreFoundation \
    -L"$MOHO_FRAMEWORKS" \
    -lavcodec.61 \
    -lavformat.61 \
    -lavutil.59 \
    -lswscale.8 \
    -lswresample.5 \
    -Wl,-rpath,"$MOHO_FRAMEWORKS"

echo "✓ 编译完成"
ls -lh "$OUTPUT"

# 检查依赖
echo ""
echo "=== 检查依赖 ==="
otool -L "$OUTPUT" | grep -E "avcodec|avformat|avutil"

# 清理
rm -f *.o
