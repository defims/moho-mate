# LEARNING_WORKFLOW - Moho Tutorial 学习流程

## 核心原则

**节点 → 骨骼 → 动作，层层增强控制力**

⚠️ **节点是地基，不牢后面都是空谈**

---

## 学习流程（标准）

### Phase 1：阅读教程

1. **阅读官方教程文档**
   - 路径：`moho_tutorials/Moho 14 Tutorial Manual/Moho 14 Tutorial Manual - X.XX.md`
   - 目标：理解教程目标和步骤

2. **整理教程步骤**
   - 创建 `Tutorial_X.XX_Steps.md`
   - 列出 GUI 操作步骤和目标效果

### Phase 2：API Mapping

1. **映射 GUI 操作到 API**
   - 创建 `Tutorial_X.XX_API_Mapping.md`
   - 对照 `references/api/*.lua_pkg` 确认函数签名

2. **验证 API 可用性**
   - 标记 ✅ 可实现、⚠️ 有限制、❌ 无法实现
   - 找替代方案（如 LoadDocument 替代 FileImport）

### Phase 3：脚本实现

1. **编写脚本**
   - 路径：`scripts/Tutorial_X.XX.lua`
   - 严格按 API 签名调用（禁止凭印象）
   - 使用模块化方法（SetBoneFromEndpoints 等）

2. **IPC 执行**
   ```bash
   moho-mate start project.moho
   moho-mate call -f scripts/Tutorial_X.XX.lua
   # IPC 不退出，可查看结果
   ```

### Phase 4：验证和调试

1. **检查项目文件**
   ```bash
   moho-mate inspect output.moho
   ```

2. **渲染预览**
   ```bash
   moho-mate render output.moho -f PNG -o /tmp/output
   ```

3. **问题排查**
   - 查看 Moho 崩溃报告：`~/Library/Logs/DiagnosticReports/Moho*.ips`
   - 对照 `SKILL.md` 常见错误表
   - 更新 API Mapping 和 SKILL.md

### Phase 5：总结和记录

1. **创建学习报告**
   - 路径：`moho_tutorials/Tutorial_X.XX_Report.md`
   - 记录：API 发现、踩坑经验、解决方案

2. **更新记忆**
   - 写入 `memory/YYYY-MM-DD.md`
   - 提升重要发现到 `MEMORY.md`

---

## Tutorial 1.04 骨骼设置专用流程

### ⚠️ SetBoneFromEndpoints 方法（v7）

**核心理念**：先确定骨骼根和尖坐标，再创建骨骼。

**优势**：
- 更直观（两点确定一线）
- 自动计算角度和长度
- 自动转换坐标系（全局 → 父骨骼局部）

**步骤**：

1. **确定骨骼根和尖的全局坐标**
   ```lua
   local rootPos = LM.Vector2:new_local()
   local tipPos = LM.Vector2:new_local()
   rootPos:Set(0, -1.0)  -- 骨骼根位置
   tipPos:Set(0, -3.0)   -- 骨骼尖位置
   ```

2. **调用 SetBoneFromEndpoints**
   ```lua
   -- 加载模块
   local home = os.getenv("HOME")
   dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

   -- 创建骨骼
   local bone = SetBoneFromEndpoints(moho, parentBone, rootPos, tipPos, nil)
   bone:SetName("MyBone")
   ```

3. **验证骨骼坐标**
   ```lua
   local actualRoot, actualTip = GetBoneEndpoints(moho, bone)
   ValidateBoneEndpoints(moho, bone, rootPos, tipPos)
   ```

### 骨骼层操作流程

1. **创建骨骼层**
   ```lua
   local boneLayer = moho:CreateNewLayer(MOHO.LT_BONE)
   boneLayer:SetName("Skeleton")
   moho:SetSelLayer(boneLayer)
   ```

2. **Frame 0：创建骨骼**
   ```lua
   moho:SetCurFrame(0, false)  -- 同步 fPos
   SetBoneFromEndpoints(moho, parentBone, rootPos, tipPos, nil)
   ```

3. **Frame > 0：调整骨骼（动画）**
   ```lua
   moho:SetCurFrame(frame, false)
   SetBoneFromEndpoints(moho, parentBone, newRootPos, newTipPos, bone)
   ```

---

## 常见错误对照表

| 错误类型 | 错误示例 | 正确做法 |
|----------|----------|----------|
| Set 返回 void | `mesh:AddLonePoint(v:Set(0,0), f)` ❌ | `v:Set(0,0); mesh:AddLonePoint(v, f)` ✅ |
| 方法归属错误 | `doc:CreateNewLayer()` ❌ | `moho:CreateNewLayer()` ✅ |
| Skeleton() 对象错误 | `moho:Skeleton()` ❌ | `boneLayer:Skeleton()` ✅ |
| 骨骼角度单位 | `fAnimAngle:SetValue(f, 90)` ❌ | `fAnimAngle:SetValue(f, math.pi/2)` ✅ |
| 动画帧值缺失 | 只设 fAnimPos ❌ | 还需设 bone.fPos:Set(x, y) ✅ |

---

## 版本

- 创建日期：2026-05-20
- 适用版本：Moho Pro 14.3