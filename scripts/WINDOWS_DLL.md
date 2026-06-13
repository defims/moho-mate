# Windows DLL 文件说明

## 概述

moho-mate 在 Windows 上需要额外的 FFmpeg DLL 文件来支持 GIF 编码。

## 文件列表

| 文件 | 大小 | 说明 |
|------|------|------|
| avfilter-10.dll | 22 MB | FFmpeg 滤镜库（GIF 调色板优化） |
| avutil-59.dll | 3.9 MB | FFmpeg 工具库（avfilter 依赖） |

## 文件位置

```
scripts/
├── moho-mate.exe
├── avfilter-10.dll    ← 必需
└── avutil-59.dll      ← 必需（avfilter 依赖）
```

## 依赖关系

```
avfilter-10.dll
├── avutil-59.dll          ← FFmpeg 工具库
├── KERNEL32.dll           ← Windows 系统
└── api-ms-win-crt-*.dll   ← Windows UCRT
```

## 获取方式

### 方法 1: 使用已编译的文件（推荐）

scripts 目录已包含预编译的 DLL 文件：
- avfilter-10.dll
- avutil-59.dll

### 方法 2: 从 FFmpeg 官方下载

1. 访问: https://www.gyan.dev/ffmpeg/builds/
2. 下载: ffmpeg-release-shared.7z
3. 解压后找到:
   - bin/avfilter-10.dll
   - bin/avutil-59.dll
4. 复制到 scripts 目录

### 方法 3: 交叉编译（在 macOS/Linux 上）

需要安装 MinGW-w64:
```bash
# macOS
brew install mingw-w64

# Linux
sudo apt install mingw-w64
```

编译步骤:
```bash
# 下载 FFmpeg n7.1 源码
git clone --depth 1 --branch n7.1 https://github.com/FFmpeg/FFmpeg.git
cd FFmpeg

# 配置（Windows x86_64）
./configure \
  --arch=x86_64 \
  --target-os=mingw64 \
  --cross-prefix=x86_64-w64-mingw32- \
  --enable-shared \
  --disable-static \
  --disable-programs \
  --disable-doc \
  --disable-x86asm \
  --enable-avfilter

# 编译
make -j8

# 输出文件
# libavfilter/avfilter-10.dll
# libavutil/avutil-59.dll
```

## 命名差异

| 平台 | 前缀 | 分隔符 | 后缀 | 示例 |
|------|------|--------|------|------|
| macOS | lib | . | .dylib | libavfilter.10.dylib |
| Windows | 无 | - | .dll | avfilter-10.dll |
| Linux | lib | . | .so.X | libavfilter.so.10 |

## 为什么需要这些文件？

1. **Moho 没有内置 libavfilter**
   - Moho 内置: avcodec, avformat, avutil, swscale, swresample
   - 缺少: avfilter

2. **GIF 编码需要 libavfilter**
   - palettegen: 生成调色板
   - paletteuse: 应用调色板
   - 高质量 GIF 输出

3. **avfilter 依赖 avutil**
   - avfilter-10.dll 需要加载 avutil-59.dll
   - 两个文件必须一起分发

## 运行时加载

Windows 上 DLL 加载顺序:
1. 应用程序目录
2. 系统目录 (C:\Windows\System32)
3. PATH 环境变量中的目录

将 DLL 放在 moho-mate.exe 同目录即可。

## 分发说明

### 完整分发

如果用户没有安装 Moho，需要分发所有 FFmpeg DLL:
- avcodec-61.dll
- avformat-61.dll
- avutil-59.dll
- swscale-8.dll
- swresample-5.dll
- avfilter-10.dll

### 最小分发（用户已安装 Moho）

如果用户已安装 Moho，只需分发:
- avfilter-10.dll
- avutil-59.dll（如果 Moho 版本不匹配）

## 版本兼容性

当前编译版本:
- FFmpeg n7.1
- libavfilter: 10.1.100
- libavutil: 59.8.100

Moho 内置版本可能略有差异，但 ABI 兼容。

## 故障排除

### DLL 加载失败

错误: "找不到 avfilter-10.dll"

解决:
1. 确保 avfilter-10.dll 在 moho-mate.exe 同目录
2. 或添加 scripts 目录到 PATH 环境变量

### avutil 加载失败

错误: "找不到 avutil-59.dll"

解决:
1. 确保 avutil-59.dll 在 moho-mate.exe 同目录
2. avfilter-10.dll 依赖 avutil-59.dll

### 版本不匹配

错误: "avutil-59.dll 版本不兼容"

解决:
1. 使用 scripts 目录中的 avutil-59.dll
2. 不要使用 Moho 内置的 avutil（版本可能不同）

---

_最后更新: 2026-06-13_
