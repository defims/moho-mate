# moho-mate 升级机制设计

> 版本: 1.2.0  
> 日期: 2026-06-10  
> 状态: P0 已实现

## 概述

moho-mate 是一个独立的 CLI 工具，需要自升级能力以支持：
- Bug 修复快速分发
- 新功能迭代
- 安全漏洞修补

## 架构

### 跨平台安装位置

```
macOS:   ~/Library/Application Support/com.maohou.moho-mate/
Linux:   ~/.local/share/moho-mate/
Windows: %APPDATA%\maohou\moho-mate\
```

**目录结构：**

```
<app-data-dir>/
├── bin/
│   ├── moho-mate          ← 二进制
│   └── moho-mate.bak      ← 上一版本（回滚用）
├── lib/
│   └── libavfilter.10.dylib
├── config.json            ← 用户配置（设置面板用）
├── packages/              ← 已安装脚本包
└── version.json           ← 版本信息
```

**代码实现：**

```rust
use dirs::data_dir;

fn app_data_dir() -> PathBuf {
    let base = data_dir().unwrap_or_default();
    
    #[cfg(target_os = "macos")]
    let path = base.join("com.maohou.moho-mate");
    
    #[cfg(target_os = "linux")]
    let path = base.join("moho-mate");
    
    #[cfg(target_os = "windows")]
    let path = base.join("maohou").join("moho-mate");
    
    path
}
```

### 组件关系

```
┌─────────────────────────────────────────────────────────┐
│                    发布源 (GitHub Release)                │
│  maohou/moho-mate releases                               │
│  ├── moho-mate-0.2.0-macos-x64.tar.gz                   │
│  ├── moho-mate-0.2.0-macos-x64.tar.gz.sha256            │
│  └── ...                                                 │
└─────────────────────────────────────────────────────────┘
                           │
                           │ 下载
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   本地安装目录                             │
│  ~/Library/Application Support/com.maohou.moho-mate/     │
│  ├── bin/                                               │
│  │   ├── moho-mate      ← 二进制                         │
│  │   └── moho-mate.bak  ← 上一版本（回滚用）              │
│  ├── config.json        ← 用户配置                       │
│  ├── packages/          ← 已安装脚本包                   │
│  └── libavfilter.10.dylib                               │
└─────────────────────────────────────────────────────────┘
                           │
                           │ wrapper.lua 硬编码路径
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   Moho 加载机制                           │
│  moho-mate start                                         │
│    → 生成 wrapper.lua（含二进制绝对路径）                  │
│    → Moho 执行 wrapper.lua                               │
│    → Lua dlopen(moho-mate) 加载模块                      │
└─────────────────────────────────────────────────────────┘
```

### 关键设计点

1. **路径传递**：`moho-mate start` 时通过 `current_exe()` 获取二进制绝对路径，写入 wrapper.lua
2. **Lua 模块加载**：macOS 通过 `-Wl,-export_dynamic` 让二进制可被 dlopen 加载
3. **升级原子性**：下载 → 校验 → 备份 → 替换 → 验证 → 清理

## 版本规范

```
MAJOR.MINOR.PATCH

MAJOR: 不兼容变更（API 重写、配置格式改变）
MINOR: 新功能（新命令、新参数）  
PATCH: Bug 修复
```

### 版本号来源

```rust
// 编译时注入
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_DATE: &str = env!("BUILD_DATE"); // build.rs 生成

fn main() {
    println!("moho-mate {} ({})", VERSION, BUILD_DATE);
}
```

## CLI 命令

### `moho-mate --version`

```bash
$ moho-mate --version
moho-mate 0.1.0 (2026-06-10)

$ moho-mate -V
moho-mate 0.1.0 (2026-06-10)
```

### `moho-mate self check-update`

```bash
$ moho-mate self check-update
当前版本: 0.1.0
最新版本: 0.2.0
更新内容:
  - feat: add mohoscripts pagination support
  - fix: search timeout issues
  - fix: default pagination to 1 page

$ moho-mate self check-update --json
{
  "current": "0.1.0",
  "latest": "0.2.0",
  "has_update": true,
  "notes": "feat: add mohoscripts pagination..."
}
```

### `moho-mate self update`

```bash
$ moho-mate self update
▶ 检查更新...
▶ 当前: 0.1.0 → 最新: 0.2.0
▶ 下载 moho-mate-0.2.0-macos-x64.tar.gz...
  ████████████████████ 100% (5.7MB)
▶ 校验 SHA256... ✓
▶ 备份当前版本 → moho-mate.bak
▶ 解压替换...
▶ 验证新版本...
✓ 已更新到 0.2.0

# 强制升级到指定版本
$ moho-mate self update --version 0.3.0

# 允许 MAJOR 升级（默认只允许 PATCH/MINOR）
$ moho-mate self update --major
⚠️ 检测到 MAJOR 版本升级: 0.5.0 → 1.0.0
  可能包含不兼容变更，确认继续？[y/N]
```

### `moho-mate self rollback`

```bash
$ moho-mate self rollback
▶ 当前: 0.2.0
▶ 备份: 0.1.0 (moho-mate.bak)
▶ 回滚中...
✓ 已回滚到 0.1.0
```

### `moho-mate init`

首次安装或配置损坏时运行，自动检测环境并生成配置：

```bash
$ moho-mate init
▶ 初始化 moho-mate...

▶ 检测 Moho 安装...
  ✓ 找到: /Applications/Moho.app

▶ 检测 Moho 配置目录...
  ✓ 找到: ~/Library/Preferences/Lost Marble/Moho Pro/14

▶ 生成配置文件...
  ✓ ~/Library/Application Support/com.maohou.moho-mate/config.json

▶ 创建必要目录...
  ✓ bin/
  ✓ packages/

✓ 初始化完成

提示:
  - 运行 'moho-mate --help' 查看命令
  - 运行 'moho-mate config' 修改设置
  - 运行 'moho-mate self update' 更新版本
```

## 发布流程

### 双仓库架构

```
maohou/moho-mate (private)     ← 代码开发，私有
maohou/moho-mate-releases (public) ← 只放 releases，公开
```

**CI 流程：**
```
私有仓库 push tag → CI 构建 → 发布到公开仓库 release
```

### GitHub Release 结构

```
maohou/moho-mate releases
├── v0.1.0
│   ├── moho-mate-0.1.0-macos-x64.tar.gz
│   │   └── moho-mate          # 单一二进制
│   ├── moho-mate-0.1.0-macos-x64.tar.gz.sha256
│   ├── moho-mate-0.1.0-macos-arm64.tar.gz
│   ├── moho-mate-0.1.0-macos-arm64.tar.gz.sha256
│   ├── moho-mate-0.1.0-windows-x64.zip
│   └── moho-mate-0.1.0-windows-x64.zip.sha256
└── v0.2.0
    └── ...
```

### 发布清单 (release.json)

```json
{
  "version": "0.2.0",
  "released_at": "2026-06-10T07:00:00Z",
  "notes": "feat: add mohoscripts pagination\nfix: search timeout",
  "assets": [
    {
      "name": "moho-mate-0.2.0-macos-x64.tar.gz",
      "url": "https://github.com/maohou/moho-mate/releases/download/v0.2.0/moho-mate-0.2.0-macos-x64.tar.gz",
      "sha256": "abc123...",
      "size": 5702804
    },
    {
      "name": "moho-mate-0.2.0-macos-arm64.tar.gz",
      "url": "...",
      "sha256": "def456...",
      "size": 5500000
    }
  ],
  "min_version": "0.1.0",
  "breaking": false
}
```

### CI/CD 流程（双仓库）

```yaml
# .github/workflows/release.yml (私有仓库)
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: macos-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: macos-arm64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: windows-x64

    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      
      - name: Package (macOS)
        if: runner.os == 'macOS'
        run: |
          tar -czf moho-mate-${{ github.ref_name }}-${{ matrix.artifact }}.tar.gz \
            -C target/${{ matrix.target }}/release moho-mate
          shasum -a 256 moho-mate-*.tar.gz > moho-mate-*.tar.gz.sha256
      
      - name: Package (Windows)
        if: runner.os == 'Windows'
        run: |
          7z a moho-mate-${{ github.ref_name }}-${{ matrix.artifact }}.zip `
            target\${{ matrix.target }}\release\moho-mate.exe
          certutil -hashfile moho-mate-*.zip SHA256 > moho-mate-*.zip.sha256
      
      # 上传 artifacts
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: |
            moho-mate-*.tar.gz
            moho-mate-*.tar.gz.sha256
            moho-mate-*.zip
            moho-mate-*.zip.sha256

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      # 下载所有 artifacts
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
      
      # 推送到公开仓库
      - name: Create Release in Public Repo
        uses: softprops/action-gh-release@v2
        with:
          repository: maohou/moho-mate-releases
          token: ${{ secrets.PUBLIC_REPO_TOKEN }}  # 需要有公开仓库写权限的 PAT
          tag: ${{ github.ref_name }}
          name: moho-mate ${{ github.ref_name }}
          body: |
            ## Changes
            
            See [private repo](${{ github.server_url }}/${{ github.repository }}) for details.
          files: |
            artifacts/**/*
          draft: false
          prerelease: false
```

## 升级策略

### 版本升级规则

```
默认行为:
  self update           → 升级到最新 PATCH 版本（同 MINOR）
  self update --minor   → 允许升级 MINOR
  self update --major   → 允许升级 MAJOR（需确认）

示例:
  当前 0.1.0
  最新 0.1.5  → self update 可升级
  最新 0.2.0  → self update --minor 可升级
  最新 1.0.0  → self update --major 可升级（需确认）
```

### 自动检查

```
策略: 每周静默检查一次
时机: moho-mate start 时检查（如果距离上次检查 > 7天）
行为: 
  - 有更新时输出提示（不自动下载）
  - 记录检查时间到 <app-data-dir>/last-check
  - 用户可禁用：moho-mate config set auto-check false
```

### 安全考虑

1. **HTTPS only** — 只从 HTTPS 下载
2. **SHA256 校验** — 必须校验通过才替换
3. **签名验证**（未来）— 使用 GPG 或 Sigstore 签名
4. **回滚保护** — 验证失败自动回滚

## 安装流程

### 首次安装

```bash
# 一行命令
# macOS/Linux
curl -fsSL https://moho-mate.maohou.com/get | bash

# Windows (PowerShell)
irm https://moho-mate.maohou.com/get.ps1 | iex

# 或直接下载二进制
# https://moho-mate.maohou.com/download
```

### install.sh 做什么

```bash
#!/bin/bash
set -e

# 1. 检测平台
OS=$(uname -s)
ARCH=$(uname -m)
case "$OS-$ARCH" in
  Darwin-x86_64)  ARTIFACT="macos-x64" ;;
  Darwin-arm64)   ARTIFACT="macos-arm64" ;;
  MINGW*-x86_64)  ARTIFACT="windows-x64" ;;
  *)              echo "不支持的平台: $OS-$ARCH"; exit 1 ;;
esac

# 2. 确定安装目录（跨平台）
case "$OS" in
  Darwin)  INSTALL_DIR="$HOME/Library/Application Support/com.maohou.moho-mate" ;;
  Linux)   INSTALL_DIR="$HOME/.local/share/moho-mate" ;;
  MINGW*|MSYS*) INSTALL_DIR="$APPDATA/maohou/moho-mate" ;;
esac

BIN_DIR="$INSTALL_DIR/bin"
mkdir -p "$BIN_DIR"

# 3. 下载最新版本
echo "▶ 下载最新版本..."
LATEST_URL="https://api.github.com/repos/maohou/moho-mate/releases/latest"
DOWNLOAD_URL=$(curl -fsSL "$LATEST_URL" | jq -r ".assets[] | select(.name | contains(\"$ARTIFACT.tar.gz\")) | .browser_download_url")

curl -fsSL "$DOWNLOAD_URL" | tar -xzf - -C "$BIN_DIR"
chmod +x "$BIN_DIR/moho-mate"

# 4. 初始化配置
"$BIN_DIR/moho-mate" init

# 5. 验证
echo ""
echo "=== 安装完成 ==="
"$BIN_DIR/moho-mate" --version
echo ""
echo "安装位置: $BIN_DIR/moho-mate"
echo "配置文件: $INSTALL_DIR/config.json"
```

## 实现优先级

| 优先级 | 任务 | 状态 |
|--------|------|------|
| P0 | `moho-mate init` 初始化配置 | ✅ 已实现 |
| P0 | config.rs 重构（跨平台路径） | ✅ 已实现 |
| P0 | `--version` 输出 | ✅ 已实现 |
| P0 | `self check-update` | ✅ 已实现 |
| P0 | `self update` 下载替换 | ✅ 已实现 |
| P0 | SHA256 校验 | ✅ 已实现 |
| P1 | GitHub Release CI | 待搭建 |
| P1 | install.sh | 待编写 |
| P2 | `self rollback` | 待实现 |
| P2 | 自动检查通知 | 待实现 |
| P3 | GPG 签名验证 | 未来 |

## 附录

### 相关文件

- `src/main.rs` — CLI 命令定义，wrapper.lua 生成
- `src/app_config.rs` — 跨平台配置管理
- `src/self_update.rs` — GitHub Release API + 下载替换
- `.github/workflows/release.yml` — 发布 CI（私有仓库）
- `maohou/moho-mate-releases` — 公开仓库（只放 releases）
- `install.sh` — 安装脚本

### 参考资料

- [Rust 自更新库](https://github.com/jaemk/self_update)
- [GitHub Release API](https://docs.github.com/en/rest/releases)
- [语义化版本](https://semver.org/lang/zh-CN/)
