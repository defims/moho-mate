# moho-mate

> Moho 的 AI CLI 助手

macOS 命令行工具，自动化 Moho 动画软件操作。

---

## 安装

### Homebrew（推荐）

```bash
brew tap defims/moho-mate
brew install moho-mate
```

### 下载安装包

从 [GitHub Releases](https://github.com/defims/moho-mate/releases) 下载最新版本：

- **macOS (Apple Silicon)**: `moho-mate-*.dmg` 或 `moho-mate-*-macos-arm64.zip`
- **macOS (Intel)**: `moho-mate-*-macos-x64.zip`

### 安装步骤

1. 下载并解压 `.zip` 或打开 `.dmg`
2. 将 `moho-mate.app` 拖拽到 `/Applications`
3. 首次运行需要右键点击 → 打开（绕过 Gatekeeper）
4. 终端输入 `moho-mate --help` 验证安装

---

## 快速开始

```bash
# 启动 IPC 并打开项目（可选）
moho-mate start [project.moho] [script.lua]

# 执行 Lua 代码
moho-mate call 'print("Hello Moho!")'

# 保存当前项目
moho-mate call 'moho:FileSave()'

# 查看 IPC 状态
moho-mate status

# 渲染项目
moho-mate render project.moho -f PNG -o /tmp/frames/

# 编码视频（使用 Moho 内置 FFmpeg）
moho-mate encode "/tmp/frame_%05d.png" video.mp4 --fps 24

# AI 对话模式
moho-mate chat -p "帮我创建一个行走的角色动画"
```

---

## 功能

| 功能 | 说明 |
|------|------|
| **IPC 启动** | 自动启动 Moho、注入脚本、建立 socket 通信 |
| **Lua 执行** | 通过 socket 向 Moho 发送 Lua 命令 |
| **渲染** | 调用 Moho API 渲染项目为 PNG 序列 |
| **编码** | 使用 Moho 内置 FFmpeg 编码为 MP4/GIF/APNG |
| **项目检查** | 查看 .moho 文件结构、图层、骨骼信息 |
| **包管理** | 脚本包管理（类似 npm） |
| **自升级** | 检查更新、下载、回滚 |
| **AI Chat** | 对话式完成动画任务 |

---

## 命令

```
moho-mate [COMMAND]

Commands:
  start        启动 IPC 服务
  call         发送 Lua 命令
  quit         退出 Moho
  status       IPC 状态
  render       渲染项目
  encode       编码视频
  inspect      查看项目信息
  config       配置管理
  gui          GUI 界面
  pkg          包管理
  mohoscripts  脚本搜索
  init         初始化配置
  self         自升级管理
  chat         AI Agent 模式
```

---

## Lua 模块

在 Moho 脚本中直接调用：

```lua
package.cpath = "/path/to/moho-mate;" .. package.cpath
local ipc = require("moho_ipc")

-- 发送命令
ipc.call("moho:FileSave()")
ipc.call("moho:LayerCount()")

-- 获取状态
local running, socket_path = ipc.status()
```

---

## 依赖

- **Moho Pro 14.4+** - 必须安装 Moho 动画软件
- **macOS** - 当前支持 macOS（x86 + Apple Silicon）

---

## 文档

- [中文文档](./README_CN.md)
- [升级机制](./docs/upgrade-mechanism.md)

---

## License

MIT