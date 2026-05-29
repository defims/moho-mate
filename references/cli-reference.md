# Moho CLI 命令参考 (v6.0)

## 安装

```bash
# 主脚本路径
~/.openclaw/workspace/skills/moho-mate/scripts/moho-mate.sh

# 添加别名（推荐）
alias moho-mate='~/.openclaw/workspace/skills/moho-mate/scripts/moho-mate.sh'
```

---

## 核心命令

### exec - 执行脚本（退出模式）

```bash
moho-mate exec script.lua              # 执行脚本，退出
moho-mate exec project.moho script.lua # 打开项目 + 执行脚本，退出
```

执行后 Moho 自动退出。

---

### serve - 启动 IPC 服务

```bash
moho-mate serve                        # 启动 IPC (默认 1 小时)
moho-mate serve -t 60                  # 启动 IPC (60 秒超时)
moho-mate serve script.lua             # 启动 + 执行脚本
moho-mate serve project.moho script.lua # 启动 + 项目 + 脚本
```

| 参数 | 说明 |
|------|------|
| `-t N` / `--timeout N` | IPC 超时（秒），默认 3600 |

**IPC 注意事项：**
- IPC 模式下默认 **不退出 Moho**，保持服务运行
- 只有发送 `ipc_quit()` 才退出 Moho
- IPC 脚本不要调用 `moho.Quit()`

---

### call - 发送命令到 IPC

```bash
moho-mate call '<lua>'                 # 发送 Lua 命令
moho-mate call -f script.lua           # 发送 Lua 文件
```

前提：IPC 服务已启动 (`moho-mate serve`)。

---

### stop - 关闭 IPC 服务

```bash
moho-mate stop                         # 关闭 IPC 服务
```

---

### status - 查看 IPC 状态

```bash
moho-mate status                       # 查看 IPC 状态
```

---

## 其他命令

### render - 无头渲染

```bash
moho-mate render <project> [options]
```

#### 主选项

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `<project>` | 项目文件 (.moho) | (必需) |
| `-f <format>` | 输出格式 | JPEG |
| `-o <path>` | 输出文件/目录 | 同项目名 |
| `--options <codec>` | 视频编码预设 | (无) |
| `--start <frame>` | 起始帧 | 项目起始帧 |
| `--end <frame>` | 结束帧 | 项目结束帧 |
| `-v` | 详细模式 | 关闭 |
| `-q` | 静默模式 | 关闭 |

**支持格式：**
- 静态：JPEG, PNG, TGA, BMP, PSD
- 视频：QT, MP4, Animated GIF

#### 渲染选项 (yes/no)

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-multithread` | 多线程渲染 | yes |
| `-halfsize` | 半尺寸渲染 (快速预览) | no |
| `-halffps` | 半帧率渲染 | no |
| `-shapefx` | 渲染形状效果 | yes |
| `-layerfx` | 渲染图层效果 | yes |
| `-aa` | 抗锯齿边缘 | yes |

#### 示例

```bash
# 基本 JPEG 渲染
moho-mate render scene.moho

# MP4 视频
moho-mate render scene.moho -f MP4 -o ~/video.mp4

# 快速预览 (半尺寸)
moho-mate render scene.moho -halfsize yes

# 渲染特定帧范围
moho-mate render scene.moho --start 10 --end 50
```

---

### draw - 绘制形状

```bash
moho-mate draw <shape> [output]
```

| 形状 | 部件 | 说明 |
|------|------|------|
| circle | 1 | 蓝色圆形 |
| bunny | 7 | 白身、粉耳、黑眼、粉鼻 |
| puppy | 7 | 金黄身、棕耳、黑眼 |

---

### open - 打开项目 (GUI)

```bash
moho-mate open <project.moho>
```

---

### info - 项目信息

```bash
moho-mate info <project.moho>
```

输出：分辨率、帧范围、帧率、图层列表（含点数、形数）

---

### config - 管理配置

```bash
moho-mate config list        # 查看配置
moho-mate config backup      # 备份配置
moho-mate config optimize    # 优化设置
moho-mate config restore <ts> # 恢复备份
```

备份位置：`~/.openclaw/workspace/moho-mate/config_backup/<timestamp>/`

---

## 配置目录

```
~/Library/Preferences/Lost Marble/Moho Pro/14/
├── Moho Pro14.user.settings  # 用户设置
└── Autosave/                  # 自动保存
```

---

## 日志目录

```
~/.openclaw/workspace/moho-mate/logs/
├── render_*.log
└── draw_*.log
```

---

## 注意事项

1. **视频格式渲染：** 无 `--options` 时，自动使用 PNG→ffmpeg 转换
2. **物理动画：** 命令行渲染不执行物理效果，需用 GUI Export Movie
3. **IPC 模式：** 脚本不要调用 `moho.Quit()`，使用 `call 'ipc_quit()'` 关闭