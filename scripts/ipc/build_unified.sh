#!/bin/bash
# 编译统一版 moho-mate（可执行文件 + Lua 库合二为一）

set -e

LUA_SRC="/tmp/lua-5.4.4/src"
FFMPEG_SRC="/tmp/ffmpeg-7.0.2"
MOHO_FRAMEWORKS="/Applications/Moho.app/Contents/Frameworks"
IPC_DIR="$HOME/.openclaw/workspace/skills/moho-mate/scripts/ipc"
SCRIPTS_DIR="$HOME/.openclaw/workspace/skills/moho-mate/scripts"
OUTPUT="$SCRIPTS_DIR/moho-mate"

cd /tmp

# 检查 Lua 源码
if [[ ! -d "$LUA_SRC" ]]; then
    echo "下载 Lua 5.4.4..."
    curl -sL https://www.lua.org/ftp/lua-5.4.4.tar.gz | tar xz
fi

# 检查 FFmpeg 头文件
if [[ ! -d "$FFMPEG_SRC" ]]; then
    echo "下载 FFmpeg 头文件..."
    git clone -c http.proxy='http://127.0.0.1:7897' --depth 1 --branch n7.0 https://github.com/FFmpeg/FFmpeg.git ffmpeg-headers
fi

# 检查 Moho 库
if [[ ! -f "$MOHO_FRAMEWORKS/libavcodec.61.dylib" ]]; then
    echo "❌ 未找到 Moho FFmpeg 库"
    exit 1
fi

echo "=== 编译 Lua 5.4.4 核心 ==="
LUA_FILES=(
    lapi.c lcode.c lctype.c ldebug.c ldump.c ldo.c lfunc.c lgc.c
    llex.c lmem.c lobject.c lopcodes.c lparser.c lstate.c lstring.c
    ltable.c ltm.c lundump.c lvm.c lzio.c lauxlib.c linit.c
    lbaselib.c lcorolib.c ldblib.c liolib.c lmathlib.c loadlib.c
    loslib.c lstrlib.c ltablib.c lutf8lib.c
)

OBJS=""
for f in "${LUA_FILES[@]}"; do
    echo "编译: $f"
    gcc -c -O2 -DLUA_COMPAT_5_3 -I"$LUA_SRC" -o "${f%.c}.o" "$LUA_SRC/$f"
    OBJS="$OBJS ${f%.c}.o"
done

echo ""
echo "=== 编译 moho_ipc.c ==="
gcc -c -O2 \
    -I"$LUA_SRC" \
    -I"$FFMPEG_SRC" \
    -o moho_ipc.o "$IPC_DIR/moho_ipc.c"

echo ""
echo "=== 编译 moho-mate.c ==="
gcc -c -O2 \
    -I"$LUA_SRC" \
    -I"$FFMPEG_SRC" \
    -o moho_mate.o "$IPC_DIR/moho-mate.c"

echo ""
echo "=== 链接统一版 moho-mate ==="
# 关键：-Wl,-export_dynamic 让 luaopen_moho_ipc 符号可被 dlopen 加载
gcc -o "$OUTPUT" moho_ipc.o moho_mate.o $OBJS \
    -framework CoreFoundation \
    -lcurl \
    -L"$MOHO_FRAMEWORKS" \
    -L"$SCRIPTS_DIR" \
    -lavcodec.61 \
    -lavformat.61 \
    -lavutil.59 \
    -lswscale.8 \
    -lswresample.5 \
    -lavfilter.10 \
    -Wl,-rpath,"$MOHO_FRAMEWORKS" \
    -Wl,-rpath,"$SCRIPTS_DIR" \
    -Wl,-export_dynamic

echo ""
echo "✓ 编译完成"
ls -lh "$OUTPUT"

# 修复库路径
MOHO_FW="/Applications/Moho.app/Contents/Frameworks"
for lib in libavcodec.61 libavformat.61 libavutil.59 libswscale.8 libswresample.5; do
    install_name_tool -change \
        "@executable_path/../Frameworks/${lib}.dylib" \
        "${MOHO_FW}/${lib}.dylib" \
        "$OUTPUT"
done

# 检查导出符号
echo ""
echo "=== 检查 luaopen_moho_ipc 符号 ==="
nm "$OUTPUT" | grep luaopen_moho_ipc

# 清理
rm -f *.o

echo ""
echo "=== 测试命令行 ==="
"$OUTPUT" --help | head -5
