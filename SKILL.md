---
name: moho-mate
description: Moho 动画软件命令行自动化工具。触发词:moho-mate、Moho 渲染、Moho 脚本、Lua 脚本执行、Moho IPC。
moho_version: Moho Pro 14.3
skill_version: 2026.06.04-v18.0
---

# Moho Mate

macOS 命令行工具,自动化 Moho Pro 14 操作。

---

## ⚠️ Agent 行为规则

**所有 Moho 工程 (.moho) 操作必须通过 moho-mate 执行,禁止直接编辑工程文件。**

### 开发流程

```
快捷命令 → moho-mate render/draw/inspect
无快捷命令 → moho-mate start project.moho script.lua
```

### IPC 规则

1. IPC 模式默认 **不退出 Moho**
2. 退出是 **可选的**:
   - `ipc_quit()` - 脚本中退出
   - `moho-mate quit` - 命令行退出
   - 不退出 - Moho 保持运行,可查看结果
3. **⚠️ IPC 脚本禁止 `moho:Quit()` → 文件损坏**
4. **⚠️ FileOpen 前必须检查文件存在** → 文件不存在会弹 GUI 阻塞 IPC
5. **⚠️ 所有 API 调用必须先查文档** → `/Applications/Moho.app/.../Lua Interfaces`
6. **⚠️ IPC 自动备份/恢复配置**（防止 autosave 污染）

### IPC 配置自动备份/恢复

**机制**：IPC 启动前备份用户配置，IPC 就绪后立即恢复。

```
IPC 启动:
  1. 关闭旧 Moho
  2. 备份 ~/Library/Preferences/Lost Marble/Moho Pro/14/ → /tmp/moho_ipc_config_backup
  3. 清空 Autosave 目录（防止之前项目污染）
  4. 创建 wrapper.lua + 启动令牌
  5. open -a Moho --args wrapper.lua
  6. 等待 IPC socket 创建
  7. ✓ IPC 就绪 → 立即恢复 Autosave 配置
```

**关键点**：
- 必须清空 Autosave，否则 Moho 启动时加载旧工程，MohoScript 不会被调用
- 必须用 `open -a Moho --args` 启动，直接运行二进制文件会崩溃
- IPC 就绪后**立即恢复**配置，让用户下次正常启动 Moho 不受影响
- 启动令牌验证，防止其他脚本意外启动 IPC

**启动命令**：
```bash
# 使用新的启动脚本（包含备份/恢复逻辑）
~/.openclaw/workspace/skills/moho-mate/scripts/moho-mate-start [project.moho] [script.lua] [--timeout 3600]

# 或使用 Rust 版本（已集成此逻辑）
moho-mate start [project.moho] [script.lua] --timeout 3600
```

### encode 视频编码

**GIF**: 使用 libavfilter palettegen/paletteuse 滤镜优化调色板（已整合到 moho_ipc.so）

```bash
moho-mate encode "/tmp/frame_%05d.png" anim.gif --fps 24
```

**MP4**: 使用内置 FFmpeg `mpeg4` 编码器

```bash
moho-mate encode "/tmp/frame_%05d.png" video.mp4 --fps 24
```

**依赖**: Moho 内置 FFmpeg + libavfilter.10.dylib（已包含）

---

### ⚠️ IPC 崩溃问题（2026-05-19 发现）

**现象**：Moho 在 IPC 断开时偶尔崩溃（SIGSEGV）

**崩溃类型**：`EXC_BAD_ACCESS (SIGSEGV) - KERN_INVALID_ADDRESS`

**可能原因**：
1. IPC socket 断开时，Moho 内部资源访问异常
2. 骨骼 API 操作触发 Moho 内部 bug
3. 频繁 IPC 调用导致资源泄漏

**规避建议**：
```bash
# 1. 避免频繁 IPC 著名，合并多个操作到一个脚本
moho-mate call -f script.lua  # 一次执行多个操作

# 2. IPC 著名后保存项目，减少数据丢失风险
moho:FileSave()

# 3. 如果需要关闭 Moho，使用 ipc_quit() 而不是 moho:Quit()
ipc_quit()  # ✅ 正确
moho:Quit() # ❌ 会损坏文件

# 4. Moho 崩溃后，检查崩溃报告
ls ~/Library/Logs/DiagnosticReports/Moho*.ips
```

**崩溃报告路径**：`~/Library/Logs/DiagnosticReports/Moho-*.ips`

### ⚠️ 核心规则：严格按 API 签名调用

**必须遵守**：

1. **所有 API 调用前，先查 `references/api/*.lua_pkg` 确认签名**
2. **禁止链式调用返回 void 的方法**
3. **禁止凭印象/类比其他语言写 API**

**历史问题（反复出现）**：

| 错误类型 | 错误示例 | 正确做法 |
|----------|----------|----------|
| Set 返回 void | `mesh:AddLonePoint(v:Set(0,0), f)` ❌ | `v:Set(0,0); mesh:AddLonePoint(v, f)` ✅ |
| 方法归属错误 | `doc:CreateNewLayer()` ❌ | `moho:CreateNewLayer()` ✅ |
| 方法归属错误 | `moho:Skeleton()` ❌ | `moho:LayerAsBone(layer):Skeleton()` ✅ |
| 属性赋值错误 | `bone.fPos = v` ❌ (被重置) | `bone.fAnimPos:SetValue(f, v)` ✅ |
| 参数类型错误 | `AddPoint(v, pointID, -1, f)` ❌ | `AddPoint(v, curveID, -1, f)` ✅ |
| 骨骼角度单位 | `fAnimAngle:SetValue(f, 90)` ❌ (角度) | `fAnimAngle:SetValue(f, math.pi/2)` ✅ (弧度) |
| 动画帧值缺失 | 只设 `fAnimPos:SetValue(f, v)` ❌ | 还需设 `bone.fPos:Set(v.x, v.y)` ✅ |
| Skeleton() 对象错误 | `moho:Skeleton()` ❌ | `boneLayer:Skeleton()` ✅ |

**后果**：
- 传入 nil → tolua 访问无效内存 → SIGSEGV 崩溃
- 参数类型错误 → Moho 内部状态异常 → 数据损坏
- Skeleton() 调用错误 → 方法不存在 → 脚本卡住/崩溃
- 动画帧值缺失 → fMovedMatrix 计算错误 → 验证失败

---

### 禁止事项

- ❌ 直接编辑 .moho 文件
- ❌ 凭印象写 Lua API（必须查文档）
- ❌ 链式调用返回 void 的方法
- ❌ 使用角度值（骨骼 fAnimAngle 用弧度）

---

## ⭐ 骨骼创建核心理念（重要！）

**创建骨骼必须使用根尖坐标 + SetBoneFromEndpoints 方法**

### 核心原则

```
骨骼创建流程：

1. 确定骨骼的根位置（root）和尖位置（tip）
   ↓
2. 调用 SetBoneFromEndpoints(moho, parent, root, tip, frame)
   ↓
3. 设置骨骼名称和强度
   ↓
4. 验证骨骼坐标是否正确
```

### 为什么用根尖坐标？

| 方法 | 问题 |
|------|------|
| 手动计算角度 | ❌ 容易出错，需要三角函数 |
| 手动设置 fPos + fAngle | ❌ 坐标系转换复杂 |
| **SetBoneFromEndpoints** | ✅ 两点确定骨骼，自动计算角度和长度 |

### 根尖坐标的优势

1. **直观**：两点确定一条线，不需要手动计算角度
2. **自动**：长度、角度、坐标系转换全部自动处理
3. **精确**：验证误差 < 0.001
4. **父子关系自动处理**：传入父骨骼对象，自动转换到局部坐标

### 正确用法

```lua
-- 加载 SetBoneFromEndpoints 模块
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

-- 创建骨骼（使用根尖坐标）
local root = LM.Vector2:new_local()
local tip = LM.Vector2:new_local()

-- Spine（根骨骼，parent = nil）
root:Set(0, -1)
tip:Set(0, 1)
local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)
spine:SetName("Spine")
spine.fStrength = 1.0

-- RightThigh（子骨骼，parent = spine）
root:Set(0.3, -1)
tip:Set(0.5, -2)
local rightThigh = SetBoneFromEndpoints(moho, spine, root, tip, nil)
rightThigh:SetName("RightThigh")
rightThigh.fStrength = 0.8
```

### ⚠️ 坐标系统匹配

**骨骼坐标直接使用矢量层全局坐标，不修改骨骼层位置！**

```lua
-- 步骤 1：分析矢量层坐标范围
local mesh = vectorLayer:Mesh()
local minX, maxX, minY, maxY = 999, -999, 999, -999
for i = 0, mesh:CountPoints() - 1 do
    local pt = mesh:Point(i)
    minX = math.min(minX, pt.fPos.x)
    maxX = math.max(maxX, pt.fPos.x)
    minY = math.min(minY, pt.fPos.y)
    maxY = math.max(maxY, pt.fPos.y)
end
local centerX = (minX + maxX) / 2
local centerY = (minY + maxY) / 2

-- 步骤 2：骨骼坐标使用全局坐标（直接从中心延伸）
root:Set(centerX, centerY - 0.3)
tip:Set(centerX, centerY + 0.3)
local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)

-- ⚠️ 错误做法：不要修改骨骼层位置
-- boneLayer.fTranslation:SetValue(0, v)  -- ❌ 会影响整个骨骼层变换
```

**SetBoneFromEndpoints 输入全局坐标，骨骼层保持默认位置 (0,0)。**

### SetBoneFromEndpoints 参数说明（v7）

```lua
SetBoneFromEndpoints(moho, parentBone, rootPos, tipPos, frame, targetBone)

-- 参数：
moho          -- ScriptInterface 对象
parentBone    -- 父骨骼对象（nil = 根骨骼）
rootPos       -- 骨骼根位置（全局坐标，Vector2）
tipPos        -- 骨骼尖位置（全局坐标，Vector2）
frame         -- 动画帧（nil = Frame 0）
targetBone    -- 目标骨骼（nil = 创建新骨骼）

-- 返回：骨骼对象
```

### 验证骨骼

```lua
-- 使用 ValidateBoneEndpoints 验证骨骼坐标
local expectedRoot = LM.Vector2:new_local()
local expectedTip = LM.Vector2:new_local()
expectedRoot:Set(0, -1)
expectedTip:Set(0, 1)

local valid = ValidateBoneEndpoints(skel, spine, expectedRoot, expectedTip, 0)
if valid then
    print("✅ Spine 验证通过")
else
    print("❌ Spine 验证失败")
end
```

### 坐标系统说明

| 坐标类型 | 说明 |
|---------|------|
| **全局坐标** | SetBoneFromEndpoints 输入坐标，以骨骼层中心为原点 |
| **局部坐标** | 子骨骼在父骨骼坐标系中的位置（自动转换） |
| **骨骼层位置** | fTranslation，影响骨骼的全局位置 |

### 模块路径

```
scripts/SetBoneFromEndpoints.lua  -- 骨骼创建模块
scripts/LaplacianSkeleton.lua     -- 2D Laplacian contraction 骨架提取
scripts/AutoBoneGenerator.lua     -- 自动骨骼生成器
scripts/BoneReparent.lua          -- 骨骼父级变更模块
scripts/IPCFileSaveAs.lua         -- IPC 保存文件模块
```

---

## 播放控制 API（Lua）

**IPC 模式播放控制（仅 Lua API）**

```lua
local ipc = require('moho_ipc')

-- 播放
ipc.play(start_frame, end_frame, fps)

-- 暂停/恢复
ipc.pause()

-- 停止
ipc.stop_play()

-- 跳转
ipc.seek(frame)

-- 状态
local s = ipc.play_status()
-- 返回: {status, status_text, current_frame, start_frame, end_frame, fps}

-- 是否播放中
local playing = ipc.is_playing()
```

### 状态值

| status | status_text | 说明 |
|--------|-------------|------|
| 0 | stopped | 已停止 |
| 1 | playing | 播放中 |
| 2 | paused | 已暂停 |

---

## ⭐ 自动骨骼生成（AutoBoneGenerator）

**从矢量层 Mesh 自动生成骨骼结构**

### 使用方法

```lua
-- 加载模块
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/AutoBoneGenerator.lua")

-- 自动生成骨骼
local result = AutoGenerateBones(moho, mesh, boneLayer, 100, 0.1, 0.05)

-- 参数：
-- moho: ScriptInterface 对象
-- mesh: 矢量层的 Mesh 对象
-- boneLayer: 骨骼层对象
-- iterations: Laplacian 收缩迭代次数（默认 100）
-- lambda: 收缩系数（默认 0.1）
-- clusterThreshold: 聚类距离阈值（默认 0.05）
```

### 核心流程

```
1. 分析矢量层坐标范围 → 计算中心
   ↓
2. Laplacian contraction 收缩点集
   ↓
3. 提取骨架节点（分支点 = 骨骼关节）
   ↓
4. 构建骨骼树（确定父子关系）
   ↓
5. SetBoneFromEndpoints 创建骨骼（直接使用全局坐标）
   ↓
6. 计算骨骼强度 fStrength
   ↓
7. 验证骨骼结构
```

### 简化版（快速生成）

```lua
-- 不使用 Laplacian contraction，直接从 mesh 中心生成骨骼
local result = QuickGenerateBones(moho, mesh, boneLayer)
```

### 输出 JSON（调试）

```lua
local json = OutputBoneStructureJSON(result)
print(json)
```

---

### API 文档

**完整函数签名在 `references/api/` 目录:**

| 文件 | 内容 |
|------|------|
| `references/api/API_INDEX.md` | 索引 + 函数签名速查 |
| `references/api/pkg_lm.lua_pkg` | 基础类型 (Vector2, Color, Matrix 等) |
| `references/api/pkg_lm_gui.lua_pkg` | GUI 组件 (Button, Dialog 等) |
| `references/api/pkg_moho.lua_pkg` | Moho 核心 (MohoDoc, Mesh, Bone 等) |
| `references/api/pkg_anime.lua_pkg` | ScriptInterface (FileOpen, CreateShape 等) |

**调用任何 API 前必须先查 lua_pkg 确认函数签名。**

---

## Quick Start

```bash
alias moho-mate='~/.openclaw/workspace/skills/moho-mate/scripts/moho-mate.sh'

# IPC 模式
moho-mate start project.moho          # 启动 + 打开项目
moho-mate call 'moho:FileSave()'      # 保存
moho-mate call 'moho:FileRender("/tmp/out.png")'  # IPC 渲染
moho-mate quit                        # 退出

# IPC 渲染
moho-mate render project.moho -f PNG -o /tmp/output
moho-mate render project.moho --start 1 --end 72

# 查看项目
moho-mate inspect project.moho
```

---

## Commands

| 命令 | 说明 |
|------|------|
| `start [project] [script]` | 启动 IPC 服务 |
| `call '<lua>'` | 发送 Lua 命令 |
| `call -f script.lua` | 发送脚本文件 |
| `quit` | 退出 Moho |
| `render <project> [options]` | IPC 渲染 |
| `draw <shape> [output]` | 绘制形状 |
| `inspect <project>` | 查看项目信息 |
| `config list/backup/restore` | 配置管理 |
| `encode <input> <output> [options]` | PNG 序列合成视频 |
| `pkg install <package>` | 安装脚本包 ✨ |
| `pkg uninstall <package>` | 卸载脚本包 ✨ |
| `pkg list` | 列出已安装包 ✨ |
| `pkg info <package>` | 显示包信息 ✨ |
| `pkg deps <package>` | 显示依赖树 ✨ |
| `pkg search <keyword>` | 搜索 registry 包 ✨ NEW |
| `pkg set-registry <url>` | 设置 registry ✨ |


### encode 参数（使用 Moho 内置 FFmpeg，无需额外依赖）✨ NEW

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `<input>` | PNG 序列路径模式 | 必填 |
| `<output>` | 输出视频路径 | 必填 |
| `--fps <n>` | 帧率 | 24 |
| `--crf <n>` | 质量 0-51（越小越好） | 23 |

**用法：**
```bash
# MP4 编码（使用 Moho 内置 FFmpeg MPEG4 编码器）
moho-mate encode "/tmp/frame_%05d.png" "/tmp/video.mp4"

# GIF 编码（使用 Moho 内置 FFmpeg GIF 编码器 + 调色板优化）
moho-mate encode "/tmp/frame_%05d.png" "/tmp/animation.gif"

# APNG 编码（动画 PNG，支持透明通道，无损）
moho-mate encode "/tmp/frame_%05d.png" "/tmp/animation.apng"
```

**格式对比：**

| 格式 | 特点 | 适用场景 |
|------|------|----------|
| GIF | 256 色调色板，文件小 | 简单动画、表情包 |
| APNG | 24 位真彩色 + Alpha 透明，无损 | 高质量动画、UI 元素 |
| MP4 | 有损压缩，文件最小 | 视频预览、分享 |

**⚠️ 重要说明：**
- MP4/GIF/APNG 都使用 Moho 内置的 FFmpeg 库，无需安装额外依赖
- 自动检测输入 PNG 分辨率
- 编码在后台线程异步执行，不阻塞 Moho
- GIF 使用 palettegen/paletteuse 滤镜优化调色板（stats_mode=diff）
- APNG 默认无限循环，支持完整 Alpha 透明通道

**Lua API（异步编码）：**
```lua
local ipc = require("moho_ipc")

-- 启动异步编码（GCD 后台线程）
ipc.encode_video(input, output, fps, crf, codec)
-- 返回: success, message

-- 查询状态
local status = ipc.encode_status()
-- 返回 table: {status, status_text, progress, output_path, error_msg}

-- 取消编码
ipc.encode_cancel()
```

**状态值：**

| status | status_text | 说明 |
|--------|-------------|------|
| 0 | idle | 未编码 |
| 1 | running | 正在编码 |
| 2 | success | 完成 |
| 3 | error | 失败 |

**注意：** 使用 Moho 内置的 `mpeg4` 编码器，无需安装系统 ffmpeg。

---

### render 视频格式处理 ✨ NEW

**动画格式（MP4/QT/MOV/GIF/APNG）自动使用 IPC 模式：**

```
render -f MP4/GIF/APNG → IPC 模式渲染 PNG → 内置 FFmpeg 异步编码 → 输出动画文件
```

**工作流程：**

1. 自动启动 IPC（如未启动）
2. IPC 模式渲染 PNG 序列（Moho GUI 正常运行，不阻塞）
3. 调用内置 FFmpeg 异步编码
4. 等待编码完成
5. 自动清理临时 PNG 序列

**优势：**

| 项目 | 说明 |
|------|------|
| 依赖 | ✅ 使用 Moho 内置 FFmpeg，无需额外安装 |
| 阻塞 | ✅ 异步编码，Moho GUI 正常运行 |
| 清理 | ✅ 自动清理临时 PNG 序列 |

**示例：**
```bash
# 视频格式自动使用 IPC 模式
moho-mate render project.moho -f MP4 --start 0 --end 72 -o /tmp/output

# GIF 动画
moho-mate render project.moho -f GIF --start 0 --end 72 -o /tmp/output

# APNG 动画（无损 + 透明）
moho-mate render project.moho -f APNG --start 0 --end 72 -o /tmp/output
```

### render 参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-f <format>` | PNG/JPEG/MP4/GIF | JPEG |
| `-o <path>` | 输出目录 | 同项目名 |
| `--start/--end <frame>` | 帧范围 | 项目设置 |
| `-halfsize yes` | 半尺寸预览 | no |

⚠️ **视频格式（MP4/QT/GIF）**：通过 IPC + FFmpeg 编码实现。

```bash
# ✅ 正确：PNG 序列
moho-mate render project.moho -f PNG --start 0 --end 72 -o output
# 然后用 ffmpeg 合成视频
ffmpeg -framerate 24 -i output/frame_%05d.png -c:v libx264 output.mp4

# ❌ 错误：直接渲染视频（崩溃）
moho-mate render project.moho -f MP4 -o output  # SIGSEGV
```

⚠️ **`-o` 必须是目录路径**,不能是具体文件名。文件名自动生成:`<前缀>_<5位帧号>.<格式>`

```bash
# 正确 ✅
moho-mate render project.moho -o ~/Desktop/output_dir
# 输出: frame1_00001.png, frame1_00002.png, ...

# 错误 ❌
moho-mate render project.moho -o ~/Desktop/output.png
```

### draw 形状

| 形状 | 说明 |
|------|------|
| circle | 蓝色圆形 |
| bunny | 白兔(7部件) |
| puppy | 金狗(7部件) |

---

## Moho API 快速参考

### 文档对象访问

```lua
local doc = moho.document  -- 文档对象
local layer = doc:Layer(0)  -- 获取图层
local meshLayer = moho:LayerAsVector(layer)  -- 转矢量层
local mesh = meshLayer:Mesh()  -- 获取 mesh
local shape = mesh:Shape(0)  -- 获取形状
```

### Moho 方法

⚠️ **重要**:`CreateNewLayer` 是 **moho 对象的方法**(ScriptInterface),不是 MohoDoc 的方法。

| 方法 | 说明 |
|------|------|
| `CreateNewLayer(layerType, undoable)` | 创建新图层 ✅ |
| `DuplicateLayer(layer)` | 复制图层 ✅ |
| `DeleteLayer(layer)` | 删除图层 |
| `FileNew()` | 新建文档 |
| `FileOpen(path)` | 打开文档 ⚠️ 文件不存在会弹 GUI |

**⚠️ FileOpen 必须先检查文件存在：**
```lua
local path = "/path/to/project.moho"
local file = io.open(path, "r")
if file then
    file:close()
    moho:FileOpen(path)
else
    print("ERROR: 文件不存在: " .. path)
    return
end
```
| `FileSave()` | 保存 |
| `FileSaveAs(path)` | 另存为 |
| `FileImport(path, mode)` | 导入 Moho/Anime 文件 ⚠️ 弹 GUI |
| `LoadDocument(path)` | 加载外部工程（返回 MohoDoc） ✅ 纯脚本 |
| `DestroyDocument(doc)` | 关闭外部工程 ✅ |
| `FileRender(path)` | IPC 渲染当前帧(.bmp/.jpg/.png/.tga) |
| `LayerAsVector(layer)` | 转矢量层 |
| `CreateShape(fill, outline, frame)` | 创建形状 |
| `SetSelLayer(layer)` | 设置当前图层 |
| `DrawingMesh()` | 获取当前图层的 Mesh |

**创建图层示例**:
```lua
-- ✅ 正确:使用 moho 对象的方法
local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)
layer:SetName("MyLayer")
moho:SetSelLayer(layer)  -- 设置为当前图层
local mesh = moho:DrawingMesh()  -- 获取 Mesh

-- ❌ 错误:不是 MohoDoc 的方法
local layer = moho.document:CreateNewLayer(MOHO.LT_VECTOR)  -- 会报错
```

### MohoDoc 方法

| 方法 | 说明 |
|------|------|
| `CountLayers()` | 图层数量 |
| `Layer(id)` | 获取图层 |
| `LayerByName(name)` | 按名获取 |
| `CountStyles()` | 样式数量 |
| `StyleByID(id)` | 获取样式 |
| `Style(name)` | 按名获取样式 |
| `Refresh()` | 刷新显示 |
| `Width()/Height()` | 文档尺寸 |

### MohoLayer 方法

| 方法 | 说明 |
|------|------|
| `Name()` | 图层名 |
| `LayerType()` | 图层类型 |
| `FreeCachedImage()` | 释放缓存 |
| `UpdateCurFrame(extended)` | 更新当前帧 |
| `SetVisible(bool)` | 设置图层可见性 |
| `IsVisible()` | 检查图层可见性 |
| `IsFullyVisible(doc, forRendering)` | 检查完整可见性 |
| `fVisibility` | 动画可见性 (AnimBool) |

### Layer Visibility API

```lua
-- 静态可见性（GUI 眼睛图标）
layer:SetVisible(false)  -- 隐藏
layer:SetVisible(true)   -- 显示

-- 动画可见性（可在时间线上动画控制）
layer.fVisibility:SetValue(frame, false)  -- 指定帧隐藏
layer.fVisibility:SetValue(frame, true)   -- 指定帧显示

-- 检查可见性
local isVisible = layer:IsVisible()
local isFullyVisible = layer:IsFullyVisible(doc, forRendering)
```

**两种方式的区别**：

| 方法 | 类型 | 用途 |
|------|------|------|
| `SetVisible(bool)` | 静态 | GUI 图层列表眼睛图标 |
| `fVisibility:SetValue(frame, bool)` | 动画 | 可在时间线上动画控制 |

---

## ⚠️ 跨工程导入（2026-05-18 验证）

### FileImport 的 GUI 限制

**问题**：`moho:FileImport(path, mode)` 会弹出 GUI 对话框，阻塞 IPC。

```lua
moho:FileImport("/path/to/file.moho", 0)
-- ❌ 弹出导入对话框，IPC 阻塞，无法继续脚本
```

**测试结论**：
- 即使导入只有 1 层的工程，也会阻塞 IPC
- 与层数无关，是 API 本身的 GUI 触发机制

### LoadDocument + DuplicateLayer 纯脚本导入 ✅

**解决方案**：使用 `LoadDocument` 加载外部工程，然后复制层到当前工程。

```lua
-- 1. 加载外部工程（不弹 GUI，返回 MohoDoc）
local extrasDoc = moho:LoadDocument("/path/to/Tutorial Extras.moho")

-- 2. 获取外部层
local frankLayer = extrasDoc:LayerByName("Frank")

-- 3. 复制到当前工程
local clonedFrank = moho.document:DuplicateLayer(frankLayer)

-- 4. 关闭外部工程（释放内存）
moho:DestroyDocument(extrasDoc)

-- 5. 将层放入目标组
moho:PlaceLayerInGroup(clonedFrank, targetLayer)
```

**API 来源**（pkg_anime.lua_pkg）：

| API | 说明 |
|-----|------|
| `moho:LoadDocument(path)` | 加载外部工程（返回 MohoDoc） |
| `doc:DuplicateLayer(layer)` | 复制层到当前工程 |
| `moho:DestroyDocument(doc)` | 关闭外部工程 |

**完整示例**：导入 Tutorial Extras 的 Frank 到 Tutorial 1.04：

```lua
local currentDoc = moho.document
local extrasDoc = moho:LoadDocument("/Applications/Moho.app/Contents/Resources/Support/Pro/Tutorials/1 - Basics/Tutorial Extras.moho")

if extrasDoc then
  local frankLayer = extrasDoc:LayerByName("Frank")
  if frankLayer then
    local clonedFrank = currentDoc:DuplicateLayer(frankLayer)
    clonedFrank:SetName("Frank")
    
    -- 放入骨骼层
    local skeletonLayer = currentDoc:LayerByName("Skeleton")
    moho:PlaceLayerInGroup(clonedFrank, skeletonLayer)
  end
  moho:DestroyDocument(extrasDoc)
end
```

**⚠️ 注意**：`PlaceLayerInGroup` 后，`MohoDoc:LayerByName("Frank")` 返回 nil。
需用 `GroupLayer:LayerByName("Frank")` 访问（见上文 PlaceLayerInGroup 说明）。

### FileSaveAs 的 GUI 限制

**问题**：`moho:FileSaveAs(path)` 也会弹出 GUI 对话框。

```lua
moho:FileSaveAs("~/Desktop/output.moho")
-- ❌ 弹出保存对话框，IPC 阻塞
```

**解决方案**：

### IPCFileSaveAs 函数（v2 - 2026-05-19 更新）✅

**位置**：`scripts/IPCFileSaveAs.lua`

**用途**：替代 `moho:FileSaveAs(path)`，避免 GUI 弹窗和 IPC 崩溃。

**使用方法**：

```lua
-- 加载模块（Moho Lua 不支持 ~，用 HOME 环境变量）
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/IPCFileSaveAs.lua")

-- 替代 FileSaveAs
IPCFileSaveAs(moho, outputPath)
```

### BoneReparent 模块（2026-05-19 新增）✅

**位置**：`scripts/BoneReparent.lua`

**用途**：设置骨骼父子关系并自动转换坐标（从 lm_reparent_bone.lua 提取）。

**使用方法**：

```lua
-- 加载模块
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/BoneReparent.lua")

-- 设置骨骼父级（自动转换坐标）
local skel = moho:Skeleton()
local bone = skel:BoneByName("RightThigh")
local spineID = skel:BoneID(skel:BoneByName("Spine"))

ReparentBone(moho, skel, bone, spineID, 0)  -- 自动计算新位置和角度
```

**可用函数**：

| 函数 | 说明 |
|------|------|
| `ReparentBone(moho, skel, bone, parentID, frame)` | 设置骨骼父级并自动转换坐标 |
| `GetBoneGlobalPos(skel, bone, frame)` | 获取骨骼全局位置 |
| `GetBoneGlobalAngle(skel, bone, frame)` | 获取骨骼全局角度（弧度） |
| `TransformToLocalPos(pos, parent, frame)` | 全局 → 父骨骼局部 |
| `TransformToGlobalPos(pos, parent, frame)` | 父骨骼局部 → 全局 |

### SetBoneFromEndpoints 模块（2026-05-19 新增）✅

**位置**：`scripts/SetBoneFromEndpoints.lua`

**用途**：给定根和尖两个坐标创建或调整骨骼（来自 lm_add_bone.lua + lm_transform_bone.lua）。

**使用方法**：

```lua
-- 加载模块
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

-- Frame 0：创建骨骼
local rootPos = LM.Vector2:new_local()
local tipPos = LM.Vector2:new_local()
rootPos:Set(0, -1)    -- 骨骼根位置（全局）
tipPos:Set(0, 1)     -- 骨骼尖位置（全局）

local bone = SetBoneFromEndpoints(moho, skel, -1, rootPos, tipPos, 0)

-- Frame > 0：调整骨骼（添加动画关键帧）
SetBoneFromEndpoints(moho, skel, -1, newRootPos, newTipPos, frame, existingBone)
```

**可用函数**：

| 函数 | 说明 |
|------|------|
| `SetBoneFromEndpoints(moho, skel, parentID, rootPos, tipPos, frame, bone)` | 给定根和尖坐标创建/调整骨骼 |
| `GetBoneEndpoints(skel, bone, frame)` | 获取骨骼的根和尖全局坐标 |
| `ValidateBoneEndpoints(skel, bone, expectedRoot, expectedTip, frame)` | 验证骨骼是否匹配给定坐标 |

**⚠️ v7 更新（2026-05-20）**：参数改为 `parentBone` 对象，更直观。

```lua
-- 旧版本（v6）：parentID = -1 或骨骼 ID
SetBoneFromEndpoints(moho, skel, -1, rootPos, tipPos, frame)

-- 新版本（v7）：parentBone = nil 或骨骼对象
local spine = SetBoneFromEndpoints(moho, nil, rootPos, tipPos, nil)  -- 根骨骼
local thigh = SetBoneFromEndpoints(moho, spine, rootPos, tipPos, nil)  -- 父骨骼 = spine
```

**核心逻辑**：

```lua
-- 1. 计算骨骼向量（尖 - 根）
boneVec = tipPos - rootPos
length = boneVec:Mag()
angle = math.atan(boneVec.y, boneVec.x)  -- 弧度值

-- 2. 转换根位置到父骨骼坐标系
if parentID >= 0 then
    invMatrix:Set(parent.fRestMatrix)  -- frame 0
    invMatrix:Invert()
    invMatrix:Transform(rootPos)
end

-- 3. Frame 0：创建骨骼
bone.fLength = length        -- ✅ 静态属性
bone.fAnimPos:SetValue(0, rootPos)    -- ✅ 动画属性
bone.fAnimAngle:SetValue(0, angle)   -- ✅ 动画属性

-- 4. Frame > 0：调整骨骼
bone.fAnimPos:SetValue(frame, newRootPos)  -- 添加关键帧
bone.fAnimAngle:SetValue(frame, newAngle)  -- 添加关键帧
```

### Tutorial 1.04 骨骼创建最佳实践

**核心理念**：先确定骨骼根和尖坐标，再调用 SetBoneFromEndpoints 创建骨骼。

**完整流程**：

```lua
-- 1. 加载模块
local home = os.getenv("HOME")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

-- 2. 创建骨骼层
local boneLayer = moho:CreateNewLayer(MOHO.LT_BONE)
boneLayer:SetName("Skeleton")
moho:SetSelLayer(boneLayer)

-- 3. 获取 Skeleton 对象
local boneLayerObj = moho:LayerAsBone(boneLayer)
local skel = boneLayerObj:Skeleton()

-- 4. 切换到 Frame 0
moho:SetCurFrame(0, false)  -- 同步 fPos

-- 5. 定义骨骼坐标（全局坐标系）
local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()

-- 脊柱（根骨骼）
root:Set(0, -1); tip:Set(0, 1)
local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)
spine:SetName("Spine"); spine.fStrength = 1.0

-- 右大腿（父骨骼 = Spine）
root:Set(0.3, -1); tip:Set(0.5, -2)
local rightThigh = SetBoneFromEndpoints(moho, spine, root, tip, nil)
rightThigh:SetName("RightThigh"); rightThigh.fStrength = 0.8

-- 右小腿（父骨骼 = RightThigh）
root:Set(0.5, -2); tip:Set(0.6, -3)
local rightShin = SetBoneFromEndpoints(moho, rightThigh, root, tip, nil)
rightShin:SetName("RightShin"); rightShin.fStrength = 0.6

-- ... 继续创建其他骨骼

-- 6. 更新骨骼矩阵
skel:UpdateBoneMatrix(-1)
```

**优势**：
- 直观：两点确定一条线，不需要手动计算角度
- 自动：角度、长度、坐标转换全部自动处理
- 准确：验证误差 < 0.001

**⚠️ 注意**：`SetBoneFromEndpoints` 会自动处理坐标系转换，输入坐标应为**全局坐标**。

**核心逻辑**（来自 lm_reparent_bone.lua）：

```lua
-- 1. 用骨骼矩阵变换到全局坐标
bone.fMovedMatrix:Transform(v)  -- 或 fRestMatrix（frame 0）

-- 2. 用新父骨骼逆矩阵变换回局部坐标
invMatrix:Set(parent.fMovedMatrix)  -- 或 fRestMatrix
invMatrix:Invert()
invMatrix:Transform(v)

-- 3. 设置新位置和角度
bone.fAnimPos:SetValue(frame, v)
bone.fAnimAngle:SetValue(frame, angle)
```

**⚠️ 崩溃问题已修复（v2）**：

旧方案使用 `os.rename` 重命名当前打开的工程 → Moho IPC 崩溃（lua_tolstring 内存无效）。

v2 改为 **复制 + 打开**，避免 rename 当前工程。

**场景自动判断**：

| 场景 | outputPath == 当前路径 | outputPath != 当前路径 |
|------|------------------------|------------------------|
| 行为 | FileSave ✅ | 复制 + FileOpen ✅ |

**场景2流程（另存为 - v2）**：

```
1. FileSave 保存当前工程
2. os.execute("cp current output")  ← 复制到目标路径
3. moho:FileOpen(output)            ← 打开副本，关闭当前

结果：当前工程关闭，副本打开，无崩溃。
```

---

**方案 1：预先复制模板**（已弃用，推荐使用 IPCFileSaveAs）

```bash
# 1. shell 复制模板到目标位置
cp "/path/to/template.moho" ~/Desktop/output.moho

# 2. Moho 打开副本
moho:FileOpen("~/Desktop/output.moho")

# 3. 脚本修改内容
-- 导入层、创建骨骼等

# 4. FileSave() 保存（不弹 GUI）
moho:FileSave()  -- 保存到副本路径
```

**方案 2**：脚本完成操作后，让用户手动 Save As

---

| 方法 | 说明 |
|------|------|
| `CountPoints()` | 点数 |
| `CountShapes()` | 形数 |
| `CountCurves()` | 曲线数 |
| `Point(id)` | 获取点 |
| `Shape(id)` | 获取形状 |
| `Curve(id)` | 获取曲线对象 |
| `CurveID(curve)` | 获取曲线 ID |
| `AddLonePoint(v, frame)` | 创建孤立起点 |
| `AppendPoint(v, frame)` | 在曲线末尾添加 |
| `AddPoint(v, curveID, segID, frame)` | **在曲线中间插入点** ✅ |
| `AddPoint(v, attachID, frame)` | 从指定点开始新曲线 |
| `WeldPoints(p1, p2, frame)` | 焊接闭合(⚠️ 需要两点相邻) |
| `DeletePoint(id)` | 删除点 |
| `SelectNone()` | 取消选择 |

---

## ⚠️ AddLonePoint/AppendPoint 行为机制(2026-05-17 验证)

**关键发现:AddLonePoint 后 CountPoints 返回 0**

这是 Moho 的设计行为,不是 bug:

| 操作 | CountPoints 返回 | 实际状态 |
|------|-----------------|----------|
| `AddLonePoint` | 0 | 点已创建,但未计入曲线(孤立点)|
| 1st `AppendPoint` | 2 | 起点 + 新点都计入曲线 |
| 2nd `AppendPoint` | 3 | 正常累加 |
| 后续 `AppendPoint` | N+1 | 正常累加 |

**原理**:
- `AddLonePoint` 创建"孤立点",尚未形成曲线结构
- `CountPoints` 只计算曲线上的点
- `AppendPoint` 将点连接形成曲线后,计数才更新
- 第一个 AppendPoint 同时将起点和新点计入 → CountPoints = 2

**官方脚本的正确做法**:
```lua
-- 1. 记录添加前的点数
local n = mesh:CountPoints()  -- 通常为 0

-- 2. 绘制所有点
mesh:AddLonePoint(v1, frame)
mesh:AppendPoint(v2, frame)
mesh:AppendPoint(v3, frame)
...

-- 3. Weld 闭合
mesh:WeldPoints(lastPt, n, frame)  -- n 是起点 ID

-- 4. 绘制完后用 Point(n), Point(n+1)... 访问新点
mesh:Point(n):SetCurvature(MOHO.SMOOTH, frame)
mesh:Point(n+1):SetCurvature(MOHO.SMOOTH, frame)
```

**⚠️ 常见错误**:
```lua
-- ❌ 错误:AddLonePoint 后立即访问点
mesh:AddLonePoint(v, frame)
mesh:Point(0):SetCurvature(MOHO.SMOOTH, frame)  -- nil 错误!

-- ✅ 正确:绘制完后再访问
mesh:AddLonePoint(v1, frame)
mesh:AppendPoint(v2, frame)
-- 现在可以用 Point(0), Point(1) 访问
```

---

---

## ⚠️ AddPoint API 关键说明

**在已有曲线上插入点**(2026-05-16 验证):

```lua
void AddPoint(LM_Vector2 &pos, int32 attachCurve, int32 attachSeg, int32 frame, bool correctBezierHandles = true, bool preserveCorners = false)
```

**参数说明:**
| 参数 | 类型 | 说明 |
|------|------|------|
| `pos` | LM_Vector2 | 新点位置 |
| `attachCurve` | int32 | **曲线 ID**(⚠️ 不是点 ID!) |
| `attachSeg` | int32 | 线段 ID(**-1 = 自动选择最近线段**) |
| `frame` | int32 | 动画帧 |

---

**获取曲线 ID:**

```lua
-- 方法 1:简单形状(推荐)
local curveID = 0  -- 第一条曲线
mesh:AddPoint(pos, curveID, -1, frame)  -- -1 自动选择最近线段

-- 方法 2:复杂形状
local numCurves = mesh:CountCurves()
local point = mesh:Point(pointID)
local numPointCurves = point:CountCurves()  -- 点连接的曲线数
if numPointCurves > 0 then
    local curve, where = point:Curve(0)
    local curveID = mesh:CurveID(curve)
end
```

---

**❌ 常见错误:使用点 ID 作为 curveID**

```lua
-- ❌ 错误:会导致 Moho 崩溃
mesh:AddPoint(v, pointID, -1, frame)  -- pointID 不是 curveID

-- ✅ 正确
mesh:AddPoint(v, 0, -1, frame)       -- curveID = 0
```

## ⚠️ Vector2 Set 返回 void（2026-05-19 发现）

**关键发现**：`LM.Vector2:Set()` 返回 **void**，不是 Vector2 对象！

**错误调用（会崩溃）**：
```lua
-- ❌ 错误：Set 返回 void，传递 nil 给 AddLonePoint
mesh:AddLonePoint(v:Set(0, 0), frame)  -- tolua 访问无效内存 → SIGSEGV
mesh:AppendPoint(v:Set(0.2, 0.2), frame)  -- 同样崩溃
```

**正确调用**：
```lua
-- ✅ 正确：先设置值，再传递对象
v:Set(0, 0)
mesh:AddLonePoint(v, frame)

v:Set(0.2, 0.2)
mesh:AppendPoint(v, frame)
```

**API 签名**（pkg_lm.lua_pkg）:
```lua
void Set(real vx, real vy)  -- 返回 void，不是 self
void AddLonePoint(LM_Vector2 pos, int32 frame)
```

**原理**：tolua 绑定在处理 `void` 返回值时，Lua 端收到 `nil`。传递 `nil` 给 Mesh API → C++ 内部访问无效内存 → 崩溃。

---

### M_Shape 属性

| 属性 | 类型 | 说明 |
|------|------|------|
| `fMyStyle` | M_Style | 自定义样式 |
| `fHasOutline` | bool | 显示描边 |
| `fHasFill` | bool | 显示填充 |
| `fInheritedStyle` | M_Style | 继承样式 |

### M_Style 属性

⚠️ **`fLineWidth` 是相对单位,不是像素!**

公式:`显示线宽 = 文档高度 × fLineWidth`

| 属性 | 类型 | 说明 |
|------|------|------|
| `fDefineLineWidth` | bool | 启用自定义线宽 |
| `fLineWidth` | real | 线宽(相对值 = 目标px / 文档高度) |
| `fDefineLineCol` | bool | 启用自定义描边色 |
| `fLineCol` | AnimColor | 描边色 |
| `fDefineFillCol` | bool | 启用自定义填充色 |
| `fFillCol` | AnimColor | 填充色 |

---

## ⚠️ 渐变填充 API(2026-05-16 验证)

```lua
-- 设置渐变类型
shape.fMyStyle:SetGradient(MOHO.GRADIENT_LINEAR, false)

-- 设置渐变颜色点
-- position: 0.0 = 起点, 1.0 = 终点
local col = LM.ColorVector:new_local()
col:Set(r, g, b, a)  -- RGBA (0-1范围)
shape.fMyStyle:SetGradientColor(position, col:AsColorStruct())
```

**渐变类型常量:**
```lua
MOHO.GRADIENT_LINEAR   -- 线性渐变
MOHO.GRADIENT_RADIAL   -- 径向渐变
```

**完整示例:**
```lua
local shape = mesh:Shape(0)
shape.fHasFill = true

-- 渐变:Linear
shape.fMyStyle:SetGradient(MOHO.GRADIENT_LINEAR, false)

-- 顶部绿色 → 中间深绿 → 底部棕色
local col = LM.ColorVector:new_local()
col:Set(0.35, 0.6, 0.25, 1.0)
shape.fMyStyle:SetGradientColor(0.0, col:AsColorStruct())

col:Set(0.25, 0.45, 0.18, 1.0)
shape.fMyStyle:SetGradientColor(0.5, col:AsColorStruct())

col:Set(0.4, 0.25, 0.15, 1.0)
shape.fMyStyle:SetGradientColor(1.0, col:AsColorStruct())
```

---

**其他样式效果:**
```lua
-- 柔化边缘
shape.fMyStyle:SetSoftEdge(0.15)

-- 描边柔化
shape.fMyStyle:SetStrokeSoftEdge(0.1)
```

### AnimColor 方法

| 方法 | 说明 |
|------|------|
| `GetValue(frame)` | 获取颜色值(返回 {r,g,b,a}) |
| `SetValue(frame, rgb_color)` | 设置颜色 |

### 图层类型常量

| 常量 | 说明 |
|------|------|
| `MOHO.LT_MESH` | 矢量网格层 |
| `MOHO.LT_IMAGE` | 图像层 |
| `MOHO.LT_BONE` | 骨骼层 |
| `MOHO.LT_GROUP` | 分组层 |
| `MOHO.LT_SWITCH` | 切换层 |

### Moho 坐标系与角度（2026-05-19 确认）

**坐标系**：
- X 正向：向右
- Y 正向：向上
- 原点：工作区中心
- 工作区默认高度：2（Y 范围约 -1 到 1）

**角度系统**（逆时针为正，⚠️ **使用弧度值**）：

| 方向 | 弧度值 | 对应角度 |
|------|--------|----------|
| 向右 | 0 | 0° |
| 向上 | π/2 ≈ 1.5708 | 90° |
| 向左 | π ≈ 3.14159 | 180° |
| 向下 | -π/2 ≈ -1.5708 | -90° |

**⚠️ 重要**：骨骼 `fAnimAngle` 使用**弧度值**，不是角度值！
```lua
-- ✅ 正确：向上（弧度）
spine.fAnimAngle:SetValue(frame, math.pi / 2)  -- ≈ 1.5708 rad

-- ❌ 错误：向上（角度）
spine.fAnimAngle:SetValue(frame, 90)  -- 实际是 90 rad ≈ 5156°！
```

**示例**：
```lua
-- 脊柱骨骼（垂直向上）
pos:Set(0, -1.0)              -- 工作区底部中心
spine.fAnimPos:SetValue(0, pos)
spine.fAnimAngle:SetValue(0, math.pi / 2)  -- π/2 rad = 向上 ✅

-- 错误示例
spine.fAnimAngle:SetValue(0, 90)  -- 90 rad ≈ 5156° ❌
```

---

### ⚠️ fPos vs fAnimPos 区别（2026-05-20 实验验证）

**核心区别：**

| 属性 | 类型 | 作用 | 更新时机 |
|------|------|------|----------|
| `fAnimPos` | AnimVec2 | 动画通道，存储关键帧 | SetValue(frame, val) |
| `fPos` | LM_Vector2 | 当前帧位置，渲染用 | Set(x, y) 或自动同步 |
| `fAnimAngle` | AnimVal | 动画通道，存储关键帧角度 | SetValue(frame, val) |
| `fAngle` | real | 当前帧角度，渲染用 | 直接赋值或自动同步 |
| `fAnimScale` | AnimVal | 动画通道，存储关键帧缩放 | SetValue(frame, val) |
| `fScale` | real | 当前帧缩放，渲染用 | 直接赋值或自动同步 |

**fMovedMatrix/fRestMatrix 计算来源：**
- 矩阵使用 **fPos/fAngle/fScale**（当前帧值）
- **不使用** fAnimPos/fAnimAngle/fAnimScale（动画通道值）

```lua
-- 矩阵计算公式（伪代码）
fMovedMatrix = Translate(fPos) × Rotate(fAngle) × Scale(fScale)
-- 注意：从 fPos/fAngle/fScale 取值，不是 fAnimPos@
```

**同步机制：**

| 场景 | fAnimPos → fPos 同步 |
|------|---------------------|
| AddBone(frame) | ✅ 自动同步，初始值一致 |
| SetValue(frame, val) | ❌ **不**更新 fPos |
| Moho GUI 切帧 | ✅ 自动同步 |
| `moho.frame = X` (IPC) | ❌ **不**同步 |
| `moho:SetCurFrame(X, ui)` (IPC) | ✅ **同步** |
| UpdateCurFrame(true) | ❌ **不**同步 fAnimPos 到 fPos |
| UpdateBoneMatrix(-1) | ❌ **不**同步，只更新矩阵 |

**⚠️ IPC 模式切帧正确做法：**

```lua
-- ❌ 错误：只改 frame 属性，不触发同步
moho.frame = 24
-- fPos 保持旧值！

-- ✅ 正确：用 SetCurFrame 触发同步
moho:SetCurFrame(24, true)   -- 同步 + 刷新 GUI
moho:SetCurFrame(24, false)  -- 同步 + 不刷新 GUI（推荐，更快）
```

**SetCurFrame API：**

```lua
moho:SetCurFrame(frame, updateUI, enableBoneDynamics)
-- frame: 目标帧号
-- updateUI: 是否刷新 GUI（默认 true）
--   - true: 同步 fPos + 刷新 Moho 界面
--   - false: 同步 fPos + 不刷新界面（IPC 推荐）
-- enableBoneDynamics: 是否启用骨骼动力学（默认 false）
```

**关键：updateUI 参数只影响 GUI 显示，不影响 fPos 同步。**

**实验验证（2026-05-20）：**

```lua
-- 实验 F: AddBone 时自动同步
bone = skel:AddBone(0)
-- 结果: fAnimPos@0 = fPos = (0,0), fAnimAngle@0 = fAngle = π/2

-- 实验 F: SetValue 不更新 fPos
bone.fAnimPos:SetValue(0, LM.Vector2(1, 1))
-- 结果: fAnimPos@0 = (1,1), fPos = (0,0) ❌ 不一致

-- 实验 A: fPos:Set 立即生效
bone.fPos:Set(1, 2)
-- 结果: fPos = (1,2) ✅ 立即生效

-- 实验 C: fMovedMatrix 依赖 fPos
bone.fPos:Set(5, 6)
skel:UpdateBoneMatrix(-1)
-- 结果: fMovedMatrix 将 (0,0) 变换到 (5,6) ✅
```

---

### ⚠️ 骨骼 API 常见错误（2026-05-20 发现）

**问题 1：Skeleton() 调用对象错误**

```lua
-- ❌ 错误：Skeleton() 是 BoneLayer 的方法，不是 moho 的
local skel = moho:Skeleton()  -- 会报错/卡住

-- ✅ 正确：通过 BoneLayer 获取 Skeleton
local boneLayer = moho:LayerAsBone(moho.layer)
local skel = boneLayer:Skeleton()
```

**问题 2：Frame > 0 动画缺少当前帧值**

骨骼矩阵（fRestMatrix/fMovedMatrix）依赖当前帧值，不是动画关键帧值。

```lua
-- ❌ 错误：只设置动画关键帧
bone.fAnimPos:SetValue(frame, pos)
bone.fAnimAngle:SetValue(frame, angle)
bone.fAnimScale:SetValue(frame, scale)
-- fMovedMatrix 仍用旧值，验证失败！

-- ✅ 正确：同时设置动画关键帧 + 当前帧值
bone.fAnimPos:SetValue(frame, pos)   -- 动画关键帧
bone.fAnimAngle:SetValue(frame, angle)
bone.fAnimScale:SetValue(frame, scale)

bone.fPos:Set(pos.x, pos.y)          -- 当前帧值（fMovedMatrix 需要）
bone.fAngle = angle
bone.fScale = scale

skel:UpdateBoneMatrix(-1)            -- 更新矩阵
```

**完整示例**：
```lua
-- Frame 0: 创建骨骼
moho.frame = 0
local bone = skel:AddBone(0)
bone.fLength = 2.0
bone.fAnimPos:SetValue(0, LM.Vector2(0, -1))
bone.fAnimAngle:SetValue(0, math.pi/2)
bone.fPos:Set(0, -1)     -- 当前帧值
bone.fAngle = math.pi/2  -- 当前帧值
bone.fScale = 1.0
skel:UpdateBoneMatrix(-1)

-- Frame 24: 动画调整
moho.frame = 24
local scale = 1.118  -- 新长度/原长度
bone.fAnimPos:SetValue(24, LM.Vector2(0, -1))
bone.fAnimAngle:SetValue(24, 0.615)  -- 新角度
bone.fAnimScale:SetValue(24, scale)
bone.fPos:Set(0, -1)    -- 当前帧值
bone.fAngle = 0.615     -- 当前帧值
bone.fScale = scale     -- 当前帧值
skel:UpdateBoneMatrix(-1)
```

---

### ⚠️ 子骨骼坐标系（2026-05-19 确认）

**规则**：子骨骼坐标**相对父骨骼**定义。

| 要素 | 定义 |
|------|------|
| **X 轴正向** | 父骨骼 0° 朝向 |
| **Y 轴正向** | 父骨骼 90° 朝向 |
| **原点** | 父骨骼根处（起点） ⚠️ |

**⚠️ 关键：子骨骼连接位置**

子骨骼要连接到父骨骼**末端**，Y 必须等于父骨骼长度！

```lua
-- ✅ 正确：Y = 父骨骼 fLength，连接到末端
local parentLength = parentBone.fLength
pos:Set(0.3, parentLength)  -- 连接到末端
childBone.fAnimPos:SetValue(frame, pos)

-- ❌ 错误：Y = 0，连接到起点（不是末端）
pos:Set(0.3, 0)  -- 在父骨骼起点，不是末端！
childBone.fAnimPos:SetValue(frame, pos)
```

| 骨骼关系 | 正确 fAnimPos.y | 说明 |
|----------|-----------------|------|
| Spine → RightThigh | `2.0` (= Spine.fLength) | 连接脊柱末端 |
| RightThigh → RightShin | `1.0` (= RightThigh.fLength) | 连接大腿末端 |

**⚠️ 缩放影响**：

父骨骼缩放（fScale）会缩放整个子骨骼坐标系：
- 子骨骼相对位置被父骨骼缩放影响
- 子骨骼显示长度也会被缩放

**示例**（Spine → RightThigh）：

```
Spine（父骨骼）:
  全局位置: (0, -1)       -- 根处
  fAnimAngle: π/2 rad = 90°（向上）
  fLength: 2.0
  fScale: 1.0
  
RightThigh（子骨骼，相对 Spine）:
  相对位置: (0.3, 2.0)    -- Y = Spine.fLength（末端） ✅
  相对角度: π/6 rad = 30° -- 相对父骨骼 0° 朝向
  fLength: 1.0           -- 子骨骼长度
```

**关键理解**：
- 子骨骼 fLength 是相对单位
- 父骨骼 fScale 缩放整个子骨骼坐标系

**⚠️ 注意**：子骨骼坐标系是**旋转坐标系**，随父骨骼角度旋转。

---

### ⚠️ 骨骼角度使用弧度值（2026-05-19 确认）

**关键规则**：骨骼 `fAnimAngle` 使用**弧度值**，不是角度值！

**常用弧度值**：

| 角度 | 弧度值 | Lua 表达式 |
|------|--------|------------|
| 0° | 0 | `0` |
| 30° | π/6 ≈ 0.5236 | `math.pi / 6` |
| 45° | π/4 ≈ 0.7854 | `math.pi / 4` |
| 60° | π/3 ≈ 1.0472 | `math.pi / 3` |
| 90° | π/2 ≈ 1.5708 | `math.pi / 2` |
| 120° | 2π/3 ≈ 2.0944 | `math.pi * 2 / 3` |
| 150° | 5π/6 ≈ 2.618 | `math.pi * 5 / 6` |
| 180° | π ≈ 3.14159 | `math.pi` |
| -30° | -π/6 ≈ -0.5236 | `-math.pi / 6` |
| -90° | -π/2 ≈ -1.5708 | `-math.pi / 2` |

**角度转弧度公式**：
```lua
弧度 = 角度 × math.pi / 180
角度 = 弧度 × 180 / math.pi
```

**示例**：
```lua
-- ✅ 正确：向上（弧度）
spine.fAnimAngle:SetValue(frame, math.pi / 2)  -- π/2 rad = 90°

-- ✅ 正确：右下方倾斜 30°
thigh.fAnimAngle:SetValue(frame, math.pi / 6)  -- π/6 rad = 30°

-- ❌ 错误：使用角度值
spine.fAnimAngle:SetValue(frame, 90)  -- 90 rad ≈ 5156°！（旋转过多）
```

---

### 骨骼属性赋值方式（2026-05-19 修正）

**⚠️ 关键修正**：骨骼有两种属性类型，设置方式不同！

**静态属性（直接赋值）**：
```lua
-- ✅ 正确：静态属性直接赋值
spineBone.fLength = 2.0      -- 骨骼长度
spineBone.fStrength = 1.0   -- 骨骼强度
spineBone.fParent = -1      -- 骨骼父级 ID
```

**动画属性（用 SetValue）**：
```lua
-- ✅ 正确：动画属性用 SetValue
local pos = LM.Vector2:new_local()
pos:Set(0, -1.0)
spineBone.fAnimPos:SetValue(frame, pos)     -- 骨骼位置
spineBone.fAnimAngle:SetValue(frame, math.pi / 2)  -- 骨骼角度（弧度值！）

-- ❌ 错误：直接赋值会被重置为 0
spineBone.fPos = pos        -- 会被重置为 (0, 0)！
spineBone.fAngle = 0        -- 会被重置为 0！

-- ❌ 错误：fAnimAngle 用角度值
spineBone.fAnimAngle:SetValue(frame, 90)  -- 90 rad ≈ 5156°！（错误）
```

**骨骼 API 类型**（pkg_moho.lua_pkg - class M_Bone）：

| 属性 | 类型 | 赋值方式 | 说明 |
|------|------|----------|------|
| `fParent` | `int32` | 直接赋值 | 骨骼父级 ID（-1 为根骨骼） ✅ |
| `fLength` | `real` | 直接赋值 | 骨骼长度 ✅ |
| `fStrength` | `real` | 直接赋值 | 骨骼强度（影响范围） ✅ |
| `fPos` | `LM_Vector2` | ❌ 直接赋值无效 | 骨骼起点位置（内部属性） |
| `fAngle` | `real` | ❌ 直接赋值无效 | 骨骼角度（内部属性） |
| `fAnimPos` | `AnimVec2` | `SetValue(frame, pos)` ✅ | 骨骼位置动画通道 |
| `fAnimAngle` | `AnimVal` | `SetValue(frame, angle)` ✅ | 骨骼角度动画通道 |

**验证结果（2026-05-19）**：
```
fPos 设置后实际值：(0.0, 0.0) ❌
fAnimPos:SetValue 实际值：(0.0, -1.0) ✅
```

**骨骼创建示例**：

```lua
local boneLayer = moho:CreateNewLayer(MOHO.LT_BONE)
boneLayer:SetName("Skeleton")
local mesh = moho:LayerAsBone(boneLayer)
local skel = mesh:Skeleton()

local frame = 0
local pos = LM.Vector2:new_local()

-- 创建脊柱骨骼（根骨骼）
local spine = skel:AddBone(frame)
spine:SetName("Spine")
spine.fParent = -1       -- 根骨骼无父级 ✅
spine.fLength = 2.0     -- 长度直接赋值 ✅
spine.fStrength = 1.0   -- 强度直接赋值 ✅

-- ⚠️ 位置和角度必须用动画通道
-- Moho 角度：0°=向右, 90°=向上（逆时针为正）
pos:Set(0, -1.0)                         -- 工作区底部中心
spine.fAnimPos:SetValue(frame, pos)     -- ✅ 位置
spine.fAnimAngle:SetValue(frame, 90)    -- ✅ 90°=向上（脊柱朝向）

-- 创建子骨骼
local thigh = skel:AddBone(frame)
thigh:SetName("RightThigh")
thigh.fParent = skel:BoneID(spine)  -- 父级是脊柱 ✅
thigh.fLength = 1.0    -- 长度直接赋值 ✅
thigh.fStrength = 0.8  -- 强度直接赋值 ✅

-- ⚠️ 子骨骼位置也必须用动画通道
-- ⚠️ 骨骼角度使用弧度值！
pos:Set(0.5, -1.5)
thigh.fAnimPos:SetValue(frame, pos)    -- ✅ 位置
thigh.fAnimAngle:SetValue(frame, math.pi / 6)  -- ✅ π/6 rad ≈ 30°（弧度）
```

---

## Lua 脚本关键

### 样式与描边

⚠️ **重要:`fLineWidth` 是相对单位,不是像素!**

**公式**:
```
显示线宽 = 文档高度 × fLineWidth
```

| 文档高度 | fLineWidth 设置 | 显示线宽 |
|----------|------------------|--------------|
| 720px | 4.0 (直接设置) | 720 × 4 = **2880px** ❌ |
| 720px | 4/720 = 0.0056 | 720 × 0.0056 = **4px** ✅ |

**正确设置线宽**:
```lua
-- ✅ 正确:根据文档高度计算
local docHeight = moho.document:Height()
local myStyle = shape.fMyStyle
myStyle.fDefineLineWidth = true
myStyle.fLineWidth = 4.0 / docHeight  -- 4/720 ≈ 4px
```

**fDefineLineWidth 行为**:
| 值 | 实际渲染效果 |
|----|------------------|
| true (默认) | 使用 fLineWidth 值 |
| false | 使用全局默认线宽 |

**CreateShape 默认值**:
- `fDefineLineWidth = true`
- `fLineWidth = 4 / 文档高度 ≈ 4px`

```lua
-- 调整描边宽度(正确方式)
local docHeight = moho.document:Height()
local myStyle = shape.fMyStyle
myStyle.fDefineLineWidth = true
myStyle.fLineWidth = 4.0 / docHeight  -- 4px

-- 或修改样式库
local style0 = doc:StyleByID(0)
style0.fDefineLineWidth = true
style0.fLineWidth = 4.0 / docHeight

-- 开关描边
shape.fHasOutline = true/false

-- 获取颜色
local fillCol = myStyle.fFillCol:GetValue(0)
print("填充: R=" .. fillCol.r .. " G=" .. fillCol.g)

-- ⚠️ 修改后必须刷新+保存
layer:FreeCachedImage()
layer:UpdateCurFrame(true)
doc:Refresh()
moho:FileSave()
```

### 设置描边颜色（2026-05-22 验证）

⚠️ **关键**：描边颜色需要通过 `fMyStyle.fLineCol:SetValue(frame, color)` 设置，不是直接赋值。

```lua
-- 启用描边
shape.fHasOutline = true

-- 通过 fMyStyle 设置描边颜色
local myStyle = shape.fMyStyle
if myStyle then
    -- 启用自定义描边色
    myStyle.fDefineLineCol = true
    
    -- 设置描边颜色（AnimColor 需要 SetValue）
    local strokeColor = LM.ColorVector:new_local()
    strokeColor:Set(1, 0, 0, 1)  -- RGBA (0-1范围)
    myStyle.fLineCol:SetValue(frame, strokeColor)
    
    -- 填充透明
    myStyle.fDefineFillCol = true
    local fillColor = LM.ColorVector:new_local()
    fillColor:Set(0, 0, 0, 0)  -- 透明
    myStyle.fFillCol:SetValue(frame, fillColor)
end
```

**⚠️ 常见错误**：

```lua
-- ❌ 错误：直接赋值无效
shape.fStrokeCol = strokeColor
shape.fLineCol = strokeColor

-- ✅ 正确：通过 fMyStyle.fLineCol:SetValue
shape.fMyStyle.fLineCol:SetValue(frame, strokeColor)
```

### 文件导入

```lua
-- 导入 Moho/Anime 工程（导入所有图层）
-- mode: 0=副本, 1=引用, 2=询问用户
moho:FileImport("/path/to/assets.moho", 0)

-- 导入 OBJ 3D 模型
moho:ImportOBJ("/path/to/model.obj")

-- 导入 EPS 矢量
moho:ImportEPS("/path/to/vector.eps")
```

**⚠️ FileImport 注意事项：**
- 导入整个工程的所有图层（作为 Group Layer）
- mode 参数只对 Moho/Anime 文件有效
- 其他格式（OBJ/EPS）mode 被忽略

**⚠️ SVG 无 Lua API**：只能通过 GUI 或第三方脚本。

### SVG 导入脚本（ss_svg_import14.lua）

**脚本位置**：
```
/Applications/Moho.app/Contents/Resources/Support/Scripts/Tool/ss_svg_import14.lua
```

**✅ 可脚本调用**（跳过 GUI 对话框）：
```lua
-- 预设属性跳过对话框
SS_SVGImport.browseFirst = false  -- 跳过文件选择
SS_SVGImport.hideDialog = true   -- 隐藏选项对话框
SS_SVGImport.filename = "/path/to/file.svg"

-- 其他可选设置
SS_SVGImport.recenter = true      -- 居中导入
SS_SVGImport.minify = true        -- 合并图层
SS_SVGImport.expandGroups = false -- 不展开分组

-- 运行导入
SS_SVGImport:Run(moho)
```

**⚠️ 注意**：
- 需要先加载脚本：`require("ss_svg_import14")` 或直接在 Moho 环境中
- 脚本名 `ScriptName = "SS_SVGImport"`
- 文件必须存在且是 .svg 扩展名

**GUI 方式**：`Scripts → Tool → SVG Import`（弹出对话框选择文件）

**支持的 SVG 元素**：
| 元素 | 说明 |
|------|------|
| `path` | 路径（所有命令 M/L/H/V/C/S/Q/T/A/Z）|
| `circle` | 圆形 |
| `ellipse` | 椭圆 |
| `rect` | 矩形（支持圆角）|
| `line` | 直线 |
| `polyline/polygon` | 多线/多边形 |
| `g` | 分组 |
| `defs` | 定义区 |
| `linearGradient/radialGradient` | 渐变 |
| `clipPath/mask` |  clipping/遮罩 |
| `use` | 引用 |
| `image` | 图片 |
| `filter` | 滤镜（模糊）|

**脚本版本**：v01.15，作者 Sam Cogheil (SimplSam)

### IPC 渲染

```lua
-- 渲染当前帧
moho:FileRender("/tmp/output.png")

-- 渲染单图层
local options = MohoLayerRenderOptions:new_local()
options.frame = 0
options.minDimension = 1280
options.maxDimension = 1280
options.imageExtension = "png"
layer:RenderLayerImage(options)
print("输出: " .. options.imagePath)
```

### 绘制闭合图形 ⚠️

**2026-05-17 验证:WeldPoints 需要两点相邻!**

正确流程:
```lua
-- 1. 添加 N+1 点(最后一个是回到起点位置的 duplicate)
local n = mesh:CountPoints()
mesh:AddLonePoint(startPos, 0)    -- 起点 (ID = n)
mesh:AppendPoint(pt2, 0)
mesh:AppendPoint(pt3, 0)
mesh:AppendPoint(pt4, 0)
mesh:AppendPoint(startPos, 0)     -- duplicate 点 (ID = n+4)

-- 2. WeldPoints(duplicate 和起点)
-- 注意:AppendPoint 顺序添加,最后点和起点相邻!
local lastPt = mesh:CountPoints() - 1
mesh:WeldPoints(lastPt, n, 0)  -- Weld → N点闭合

-- 3. 选择所有点
mesh:SelectNone()
for i = 0, mesh:CountPoints() - 1 do
    mesh:Point(i).fSelected = true
end

-- 4. CreateShape
moho:CreateShape(true, false, 0)
```

**关键理解:**
- `AppendPoint` 顺序添加 → 最后点与起点相邻 → WeldPoints 成功
- 如果不添加 duplicate 点,最后点与起点不相邻 → WeldPoints 失败 → CreateShape 失败

⚠️ `SelectConnected()` 不稳定,必须手动选择点

### 绘制直线边矩形 ⚠️

**2026-05-24 验证:曲率设为0才能得到直线边**

**问题现象（2026-05-25）：**
- 绘制的矩形显示成椭圆/圆角矩形
- 原因：`AppendPoint` 默认创建贝塞尔曲线边（带控制手柄）
- 解决：设置曲率为 0

**完整函数示例：**

```lua
-- 绘制矩形边界框的完整函数
local function drawBoundingBox(mesh, moho, minX, maxX, minY, maxY, r, g, b, frame, docHeight)
    local v = LM.Vector2:new_local()
    local startID = mesh:CountPoints()
    
    -- 添加5个点（第5个回到起点）
    v:Set(minX, maxY); mesh:AddLonePoint(v, frame)  -- 左上
    v:Set(maxX, maxY); mesh:AppendPoint(v, frame)   -- 右上
    v:Set(maxX, minY); mesh:AppendPoint(v, frame)   -- 右下
    v:Set(minX, minY); mesh:AppendPoint(v, frame)   -- 左下
    v:Set(minX, maxY); mesh:AppendPoint(v, frame)   -- 回到左上（duplicate）
    
    -- Weld 闭合（两点必须相邻）
    local n = mesh:CountPoints()
    mesh:WeldPoints(n - 1, startID, frame)
    
    -- ⚠️ 关键：设置曲率为0（直线边）
    for i = startID, n - 2 do
        local pt = mesh:Point(i)
        pt:SetCurvature(0, frame)
        pt.fSelected = true
    end
    
    -- 创建形状
    local shapeID = moho:CreateShape(false, true, frame)
    
    if shapeID >= 0 then
        local shape = mesh:Shape(shapeID)
        
        -- 设置轮廓颜色（使用 AnimColor）
        shape.fDefineLineCol = true
        shape.fLineCol = LM.ColorVector:new_local()
        shape.fLineCol:Set(r, g, b, 1.0)
        
        -- 设置线宽（相对单位 = 像素 / 文档高度）
        shape.fDefineLineWidth = true
        shape.fLineWidth = 3 / docHeight  -- 3 像素
        
        -- 无填充
        shape.fDefineFillCol = false
    end
    
    mesh:SelectNone()
    return shapeID
end
```

**关键理解:**
- 默认 `AppendPoint` 创建的是曲线边（带控制手柄）
- `pt:SetCurvature(0, frame)` 将曲率设为0 = 直线边
- `ResetControlHandles()` 也可以达到类似效果

**线宽计算：**
```lua
-- 线宽是相对单位，需要根据文档高度计算
local docHeight = doc:Height()  -- 通常 720
local lineWidth = pixels / docHeight
-- 3 像素线宽: lineWidth = 3 / 720 = 0.00417
```

**简化版（无颜色设置）：**

```lua
-- 创建矩形点
local v = LM.Vector2:new_local()
local startID = mesh:CountPoints()

v:Set(minX, minY); mesh:AddLonePoint(v, frame)
v:Set(maxX, minY); mesh:AppendPoint(v, frame)
v:Set(maxX, maxY); mesh:AppendPoint(v, frame)
v:Set(minX, maxY); mesh:AppendPoint(v, frame)
v:Set(minX, minY); mesh:AppendPoint(v, frame)  -- duplicate

-- Weld 闭合
mesh:WeldPoints(mesh:CountPoints() - 1, startID, frame)

-- 关键：设置曲率为0（直线边）
for i = startID, mesh:CountPoints() - 2 do
    local pt = mesh:Point(i)
    pt:SetCurvature(0, frame)  -- 曲率=0 = 直线
    pt.fSelected = true
end

-- 创建形状
moho:CreateShape(false, true, frame)
```

**API:**
| 方法 | 说明 |
|------|------|
| `pt:SetCurvature(val, frame)` | 设置点曲率，0=直线 |
| `pt:ResetControlHandles(frame)` | 重置控制手柄 |
| `mesh:SetCurveInterpretation(interp)` | 全局曲线解释模式 |

### 坐标设置 ⚠️

**重要:不能用 Lua table 直接设置坐标!**

```lua
-- ❌ 错误:直接用 table
mesh:AddLonePoint({x=1.0, y=2.0}, frame)
-- 结果:坐标变成 e-38(几乎为 0),所有点挤在原点

-- ✅ 正确:使用 LM.Vector2
local v = LM.Vector2:new_local()
v:Set(1.0, 2.0)
mesh:AddLonePoint(v, frame)
-- 结果:坐标正常
```

**原因:**
- Moho API 需要 tolua 包装的对象,不是 Lua 原生 table
- `{x=1, y=2}` 是 Lua table,不是 Vector2 对象
- API 接收后内部指针错误,导致坐标异常

**经验教训 (2026-05-16):**
- 所有 Moho API 的坐标参数必须用 `LM.Vector2:new_local()`
- 创建后调用 `v:Set(x, y)` 设置值

---

## 知识库

### Moho 脚本执行模式

| 模式 | 启动方式 | 脚本结构 | 退出方式 |
|------|----------|----------|----------|
| **命令行模式** | `Moho script.lua` | `function MohoScript(moho) ... end` | `moho:Quit()` (可选) |
| **IPC 模式** | `moho-mate start` + `call` | 直接发送 Lua 代码片段 | `ipc_quit()` (可选) |

**两种模式退出都是可选的**:
- 不退出 → Moho 保持运行,可查看结果
- 退出 → Moho 关闭,回到命令行

### IPC 关键发现 ⚠️

**tolua ScriptInterface 对象不能跨执行上下文保存!**

| 问题 | 原因 | 解决方案 |
|------|------|----------|
| `moho.document` 是 nil | FileNew 前保存的 moho 对象 | 每次用 ScriptInterfaceHelper 获取 |
| 操作不持久化 | 直接使用保存的 `_G.moho` | 每次调用 `helper:MohoObject()` |

**正确的 IPC 命令执行方式:**
```lua
-- ❌ 错误:保存 moho 后直接用
_G.moho = moho  -- FileNew 前
moho.document   -- nil

-- ✅ 正确:每次获取当前状态
_G.ipc_execute = function(cmd)
    local helper = MOHO.ScriptInterfaceHelper:new_local()
    local moho = helper:MohoObject()  -- 当前文档状态
    -- moho.document 存在 ✅
    helper:delete()
    -- 执行命令...
end
```

**关键点:**
1. `moho.document` 在 FileNew/FileOpen 后才存在
2. tolua 对象是 Lua 包装,内部 C++ 状态可能变化
3. ScriptInterfaceHelper 每次返回当前状态的正确引用

---

**IPC 修复历史 (2026-05-16):**

| 版本 | 问题 | 修复 |
|------|------|------|
| v1-v3 | Socket 未创建 | `ipc.start()` 先执行,再 `FileNew()` |
| v4 | `moho.document` nil | 每次用 `ScriptInterfaceHelper:MohoObject()` |

### 文件格式 (.moho)

ZIP 压缩包:`Project.mohoproj` (JSON) + `preview.jpg`

### 建模理念

```
节点 → 骨骼 → 动作
(层层增强控制力)
```

⚠️ **节点是地基,不牢后面都是空谈。**

### 物理动画

⚠️ 物理模拟只在 GUI 播放生效,命令行渲染无效

### Autosave 路径

```
~/Library/Preferences/Lost Marble/Moho Pro/14/Autosave/
```

### 官方文档

- User Manual: https://manual.lostmarble.com/
- Lua API: https://mohoscripting.com
- 本地 API: `/Applications/Moho.app/.../Lua Interfaces`

---

## 版本

- Moho: Pro 14.3
- Lua: 5.4
- moho-mate: v7.13
---

## 图层顺序调整 API(2026-05-17 验证)

### PlaceLayerBehindAnother

**调整根图层顺序**:

```lua
-- 将 movingLayer 放到 targetLayer 后面(下层)
moho:PlaceLayerBehindAnother(movingLayer, targetLayer)

-- 示例:
-- Before: [LayerA, LayerB, LayerC] (C 在顶层)
moho:PlaceLayerBehindAnother(layerC, layerA)
-- After: [LayerC, LayerA, LayerB] (C 在底层)
```

### PlaceLayerInGroup

**将图层放入 GroupLayer**:

```lua
moho:PlaceLayerInGroup(layer, groupLayer)

-- 示例:
local group = moho:CreateNewLayer(MOHO.LT_GROUP)
local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)
moho:PlaceLayerInGroup(layer, group)
```

**⚠️ 关键发现（2026-05-20）**：

`PlaceLayerInGroup` 成功后，图层进入 GroupLayer 内部，但 `MohoDoc:LayerByName()` **无法找到**它！

**原因**：`MohoDoc:LayerByName()` 只查找文档根图层，不查找 GroupLayer 内部。

**正确访问方法**：使用 `GroupLayer:LayerByName()` 或 `GroupLayer:Layer(i)`。

```lua
-- ❌ 错误：MohoDoc 找不到 GroupLayer 内的图层
local frank = doc:LayerByName("Frank")  -- 返回 nil（Frank 在 Skeleton 内）

-- ✅ 正确：用 GroupLayer 方法访问子图层
local skeleton = doc:LayerByName("Skeleton")
local boneLayer = moho:LayerAsBone(skeleton)  -- BoneLayer 继承 GroupLayer

-- 方法1：LayerByName
local frank = boneLayer:LayerByName("Frank")  -- ✅ 找到

-- 方法2：遍历子图层
print("子图层数: " .. boneLayer:CountLayers())
for i = 0, boneLayer:CountLayers() - 1 do
    local layer = boneLayer:Layer(i)
    print("  [" .. i .. "] " .. layer:Name())
end
```

**GroupLayer API（pkg_moho.lua_pkg）**：

| 方法 | 说明 |
|------|------|
| `CountLayers()` | 子图层数量 |
| `Layer(id)` | 按索引获取子图层 |
| `LayerByDepth(id)` | 按深度获取 |
| `LayerByName(name)` | 按名称获取 |
| `IsMyChild(layer)` | 判断是否为子图层 |
| `IsExpanded()` | 是否展开 |
| `Expand(bool)` | 设置展开状态 |

**继承关系**：
- `BoneLayer : public GroupLayer` → 骨骼层是 GroupLayer 的子类
- `SwitchLayer : public GroupLayer` → Switch 层也是 GroupLayer 的子类
- `ParticleLayer : public GroupLayer` → 粒子层也是 GroupLayer 的子类

所以 BoneLayer、SwitchLayer、ParticleLayer 都可以使用上述 GroupLayer 方法。

### DuplicateLayer 注意事项（2026-05-20 验证）

**DuplicateLayer 会自动添加名称后缀**：

当目标文档已有同名图层时，Moho 会自动添加后缀避免冲突。

```lua
-- 文档已有 Layer 1
local doc = moho.document
local extrasDoc = moho:LoadDocument("Tutorial Extras.moho")
local frankLayer = extrasDoc:LayerByName("Frank")

-- 复制到当前文档
local clonedFrank = doc:DuplicateLayer(frankLayer)
print(clonedFrank:Name())  -- "Frank 2"（自动添加后缀）

-- 重命名回原名
clonedFrank:SetName("Frank")  -- ✅ 可以重命名
```

**推荐流程**：

```lua
-- 方法 1：导入后立即重命名
local clonedFrank = doc:DuplicateLayer(frankLayer)
clonedFrank:SetName("Frank")  -- 重命名

-- 方法 2：先删除默认 Layer 1，避免名称冲突
local layer1 = doc:LayerByName("Layer 1")
if layer1 then moho:DeleteLayer(layer1) end
local clonedFrank = doc:DuplicateLayer(frankLayer)  -- 名称就是 Frank
```

### GetLayerOrdering(组内图层顺序动画)

```lua
-- 启用组内图层顺序动画
local group = moho:LayerAsGroup(groupLayer)
group:EnableLayerOrdering(true)

-- 获取 AnimString 设置顺序
local order = group:GetLayerOrdering()
order:SetValue(frame, "LayerC,LayerA,LayerB")
```

**注意**：`PlaceLayerBehindAnother` 支持跨层级移动，可将组内图层移到文档根。


---

## Copy/Paste 形状 API（2026-05-17 验证）

**复制形状流程**：

```lua
-- 1. 选择形状
mesh:Shape(shapeID).fSelected = true

-- 2. 复制到剪切板
moho:Copy()

-- 3. 粘贴（副本在同一图层）
moho:Paste()
```

**验证结果**：
- Before: Points=2, Shapes=1
- After:  Points=4, Shapes=2 ✅ 形状被复制


---

## IPC ScriptInterface 最佳实践（2026-05-17 修正）

**关键问题**：tolua ScriptInterface 对象不能跨执行上下文保存。

**问题原因**：
- FileNew 前保存的 moho 对象，document 会是 nil
- tolua 对象是 Lua 包装，内部 C++ 状态可能变化
- 直接使用保存的 `_G.moho` 会操作不持久化

**正确用法**：
```lua
-- ✅ 推荐：每次用 ScriptInterfaceHelper 获取
moho:FileNew()  -- 或 FileOpen
local helper = MOHO.ScriptInterfaceHelper:new_local()
local moho = helper:MohoObject()  -- 获取当前文档状态
helper:delete()

local doc = moho.document  -- document 存在 ✅
local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)
```

**关键点**：
1. `moho.document` 在 FileNew/FileOpen 后才存在
2. ScriptInterfaceHelper 每次返回当前状态的正确引用
3. helper:delete() 必须显式释放

**其他属性**：
- `moho.layer` - 当前选中图层
- `moho.frame` - 当前帧（默认 0）
- `moho:DrawingMesh()` - 获取当前图层 Mesh
- `moho:LayerAsVector(layer):Mesh()` - 获取指定图层 Mesh



---

## 脚本包管理（Script Package Manager）✨ NEW

类似 npm/pnpm 的 Moho 脚本包管理系统。

### 包存储位置

```
~/Library/Application Support/com.maohou.moho-mate/packages/{name}/{version}/
```

### 快速开始

```bash
# 安装本地包
moho-mate pkg install ./my-package.zip

# 列出已安装的包
moho-mate pkg list

# 查看包信息
moho-mate pkg info my-package

# 卸载包
moho-mate pkg uninstall my-package
```

### package.json 规范

```json
{
  "name": "@maohou/moho-sdk",
  "version": "1.0.0",
  "description": "Moho SDK",
  "main": "Scripts/Modules/init.lua",
  "files": [
    "Scripts/Tool/my_tool.lua",
    "Scripts/Tool/my_tool.png",
    "Scripts/Modules/init.lua"
  ],
  "dependencies": {
    "@maohou/json": "^1.0.0"
  },
  "moho": {
    "tools": [
      { "id": "my_tool", "name": "My Tool" }
    ]
  }
}
```

### 字段说明

| 字段 | 必填 | 说明 |
|------|:----:|------|
| `name` | ✅ | 包名（支持 @org/name 格式） |
| `version` | ✅ | 版本号 |
| `main` | ❌ | 主入口（被依赖时需要） |
| `files` | ✅ | 文件清单 |
| `dependencies` | ❌ | 依赖包 |
| `moho.tools` | ❌ | Tool 脚本清单 |

### 引用文件机制

安装后，包存储在 `com.maohou.moho-mate/packages/` 目录，
用户内容文件夹只放引用文件（loadfile 方式）。

```lua
-- Tool/my_tool.lua（引用文件）
-- 由 moho-mate 自动生成

-- 添加本包和依赖的 Modules 到 package.path
local path = ".../Scripts/Modules/?.lua"
if not package.path:find(path, 1, true) then
    package.path = path .. ";" .. package.path
end

-- 加载实际脚本
return loadfile("实际路径")(...)
```

### 配置 Registry

```bash
# 查看当前配置
moho-mate pkg set-registry --default

# 设置自定义 registry
moho-mate pkg set-registry https://registry.npmjs.org
```

默认 registry：`https://mirrors.cloud.tencent.com/npm`

