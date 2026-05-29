# FFmpeg 安装指南

## 问题

`/usr/local/bin/ffmpeg` 依赖损坏：
```
dyld: Library not loaded: '/usr/local/opt/openssl/lib/libssl.1.0.0.dylib'
```

## 解决方案

### 方案 1：使用 brew 重装（推荐）

```bash
# 清理锁文件
rm -f /usr/local/var/homebrew/locks/*.lock

# 重新安装
brew reinstall ffmpeg
```

如果网络慢，可能需要等待较长时间。

### 方案 2：使用静态编译版本（更快）

```bash
# 下载静态编译的 ffmpeg
cd ~/Downloads
curl -L https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip -o ffmpeg.zip
unzip ffmpeg.zip
chmod +x ffmpeg
sudo mv ffmpeg /usr/local/bin/

# 验证
ffmpeg -version
```

### 方案 3：使用 Moho 内置

如果 Moho 有内置 ffmpeg，可以添加路径：

```bash
# 检查 Moho 是否有 ffmpeg
ls -la "/Applications/Moho.app/Contents/MacOS/" | grep ffmpeg
```

如果有，在 `moho_ipc.c` 中取消注释相关路径。

## 验证

安装后验证：
```bash
ffmpeg -version | head -1
```

应该显示类似：
```
ffmpeg version 7.0 Copyright (c) 2000-2024 the FFmpeg developers
```
