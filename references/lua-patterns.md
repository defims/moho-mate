# Moho Lua 脚本模式

## 脚本模板

### 新建文档脚本

```lua
function MohoScript(moho)
    -- 1. 创建新文档（必需）
    moho:FileNew()
    
    -- 2. 获取 Mesh
    local mesh = moho:Mesh()
    if (mesh == nil) then
        print("ERROR: mesh is nil")
        moho:Quit()
        return
    end
    
    -- 3. 绘图代码...
    
    -- 4. 保存并退出（必需）
    moho:FileSaveAs("/path/to/output.moho")
    moho:Quit()
end
```

### 修改项目脚本

```lua
function MohoScript(moho)
    -- 不调用 FileNew()，项目已打开
    
    local mesh = moho:Mesh()
    if (mesh == nil) then
        print("ERROR: mesh is nil")
        moho:Quit()
        return
    end
    
    print("Current points: " .. mesh:CountPoints())
    
    -- 修改代码...
    
    moho:FileSave()  -- 使用 FileSave()，不是 FileSaveAs()
    moho:Quit()
end
```

---

## 核心技巧

### 1. WeldPoints 闭合曲线

**必须焊接首尾点**，否则 `CreateShape` 返回 -1。

```lua
mesh:WeldPoints(mesh:CountPoints() - 1, startPt, 0)
```

### 2. 手动选择点

`SelectConnected()` 不稳定，必须手动选择：

```lua
mesh:SelectNone()
for i = startPt, mesh:CountPoints() - 1 do
    mesh:Point(i).fSelected = true
end
local shapeId = moho:CreateShape(true, false, 0)
```

### 3. 绘制椭圆函数

```lua
local function DrawEllipse(cx, cy, rx, ry, numPts, r, g, b)
    local startPt = mesh:CountPoints()
    for i = 0, numPts - 1 do
        local angle = i * math.pi * 2 / numPts
        v.x = cx + rx * math.cos(angle)
        v.y = cy + ry * math.sin(angle)
        if i == 0 then 
            mesh:AddLonePoint(v, 0)
        else 
            mesh:AppendPoint(v, 0) 
        end
    end
    mesh:WeldPoints(mesh:CountPoints() - 1, startPt, 0)
    mesh:SelectNone()
    for i = startPt, mesh:CountPoints() - 1 do
        mesh:Point(i).fSelected = true
    end
    local shapeId = moho:CreateShape(true, false, 0)
    if shapeId >= 0 then
        local shape = mesh:Shape(shapeId)
        shape.fHasFill = true
        shape.fMyStyle.fDefineFillCol = true
        local col = LM.ColorVector:new_local()
        col:Set(r, g, b, 1.0)
        shape.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())
    end
    mesh:SelectNone()
end
```

**参数：**
- `cx, cy` - 圆心坐标
- `rx, ry` - X/Y 半径
- `numPts` - 点数（16=圆形，8=简单）
- `r, g, b` - 填充颜色 (0.0-1.0)

---

## 创建形状步骤

1. `mesh:SelectNone()` — 清空选择
2. `AddLonePoint` + `AppendPoint` — 添加点
3. `WeldPoints` — 闭合曲线
4. **手动选择所有点** `mesh:Point(i).fSelected = true`
5. `CreateShape(true, false, 0)` — 创建形状

---

## 常见错误

| 问题 | 原因 | 解决 |
|------|------|------|
| 文件未生成 | 缺少 `moho:FileNew()` | 脚本开头调用 |
| CreateShape 返回 -1 | 曲线未闭合 | 调用 `WeldPoints` |
| Mesh 返回 nil | 未创建文档 | 先调用 `moho:FileNew()` |
| SIGSEGV 崩溃 | `moho:Quit()` 时机错误 | 确保正确初始化 |
| 选择失败 | `SelectConnected()` 不稳定 | 手动选择点 |

---

## 物理动画限制

- 物理模拟只在 GUI 播放时生效
- 命令行渲染**不执行**物理效果
- 必须用 GUI Export Movie 获得完整动画