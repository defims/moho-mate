# Moho Lua API Index

**来源**: `/Applications/Moho.app/Contents/Resources/Support/Pro/Extra Files/Lua Interfaces/`

---

## 文件概览

| 文件 | 内容 | 大小 |
|------|------|------|
| `pkg_lm.lua_pkg` | 基础类型 (Vector2, Vector3, Color, Matrix 等) | 8KB |
| `pkg_lm_gui.lua_pkg` | GUI 组件 (Button, TextList, Dialog 等) | 17KB |
| `pkg_moho.lua_pkg` | Moho 核心对象 (MohoDoc, MohoLayer, Mesh, Bone 等) | 55KB |
| `pkg_anime.lua_pkg` | ScriptInterface, MohoView, 事件处理 | 13KB |

---

## 核心模块

### LM 基础类型 (`pkg_lm.lua_pkg`)

| 类型 | 说明 |
|------|------|
| `LM.Vector2` | 2D 向量 (x, y) |
| `LM.Vector3` | 3D 向量 (x, y, z) |
| `LM.ColorVector` | RGBA 颜色向量 (r, g, b, a) 0-1 范围 |
| `rgb_color` | RGBA 颜色结构 (r, g, b, a) 0-255 范围 |
| `LM.Matrix` | 3x3 变换矩阵 |
| `LM.String` | 字符串包装 |
| `LM.Point` | 整数点 (x, y) |
| `LM.Rect` | 整数矩形 (left, top, right, bottom) |
| `LM.BBox` | 边界框 |
| `LM.Math` | 数学函数 |
| `LM.ColorOps` | 颜色操作 |

### LM.GUI 组件 (`pkg_lm_gui.lua_pkg`)

| 类型 | 说明 |
|------|------|
| `LM.GUI.Button` | 按钮 |
| `LM.GUI.TextList` | 文本列表 |
| `LM.GUI.StaticText` | 静态文本 |
| `LM.GUI.TextEdit` | 文本输入框 |
| `LM.GUI.Slider` | 滑块 |
| `LM.GUI.CheckBox` | 复选框 |
| `LM.GUI.RadioButton` | 单选按钮 |
| `LM.GUI.ColorPicker` | 颜色选择器 |
| `LM.GUI.Menu` | 菜单 |
| `LM.GUI.Dialog` | 对话框 |

### MOHO 核心对象 (`pkg_moho.lua_pkg`)

| 类型 | 说明 |
|------|------|
| `MohoDoc` | 文档对象 |
| `MohoLayer` | 图层基类 |
| `MeshLayer` | 矢量图层 |
| `ImageLayer` | 图像图层 |
| `GroupLayer` | 分组图层 |
| `BoneLayer` | 骨骼图层 |
| `SwitchLayer` | 切换图层 |
| `ParticleLayer` | 粒子图层 |
| `M_Mesh` | 网格数据 |
| `M_Shape` | 形状 |
| `M_Style` | 样式 |
| `M_Bone` | 骨骼 |
| `M_Skeleton` | 骨骼系统 |
| `AnimChannel` | 动画通道基类 |
| `AnimVal` | 数值动画通道 |
| `AnimVec2` | 2D 向量动画通道 |
| `AnimVec3` | 3D 向量动画通道 |
| `AnimColor` | 颜色动画通道 |
| `AnimBool` | 布尔动画通道 |
| `AnimString` | 字符串动画通道 |
| `InterpSetting` | 插值设置 |

### MOHO.ScriptInterface (`pkg_anime.lua_pkg`)

| 方法 | 说明 |
|------|------|
| `CreateNewLayer(layerType, undoable)` | 创建图层 |
| `DuplicateLayer(layer, byReference)` | 复制图层 |
| `DeleteLayer(layer)` | 删除图层 |
| `PlaceLayerInGroup(child, group, top, isUndoable)` | 放入分组 |
| `LayerAsVector(layer)` | 转矢量层 |
| `LayerAsImage(layer)` | 转图像层 |
| `LayerAsBone(layer)` | 转骨骼层 |
| `LayerAsGroup(layer)` | 转分组层 |
| `Mesh()` / `DrawingMesh()` | 获取网格 |
| `Skeleton()` | 获取骨骼系统 |
| `CreateShape(filled, behindNeighbor, frame)` | 创建形状 |
| `FileNew()` / `FileOpen(path)` / `FileSave()` | 文件操作 |
| `FileRender(path)` | IPC 渲染 |
| `LoadDocument(path)` | 加载外部工程 |
| `DestroyDocument(doc)` | 关闭外部工程 |
| `SetCurFrame(frame, updateUI, enableBoneDynamics)` | 设置当前帧 |

---

## 常用 API 函数签名速查

### 创建对象

```lua
-- Vector2/Vector3/ColorVector
local v = LM.Vector2:new_local()
v:Set(x, y)

local v3 = LM.Vector3:new_local()
v3:Set(x, y, z)

local col = LM.ColorVector:new_local()
col:Set(r, g, b, a)  -- 0-1 范围
col:AsColorStruct()  -- 转为 rgb_color
```

### Mesh 操作

```lua
-- M_Mesh 方法
mesh:AddLonePoint(pos, frame)           -- 创建孤立起点
mesh:AppendPoint(pos, frame)            -- 在曲线末尾添加点
mesh:AddPoint(pos, curveID, segID, frame) -- 在曲线中间插入点
mesh:WeldPoints(p1, p2, frame)          -- 焊接两点（需相邻）
mesh:CountPoints()                      -- 点数
mesh:Point(id)                          -- 获取点对象
mesh:CountShapes()                      -- 形状数
mesh:Shape(id)                          -- 获取形状
mesh:Curve(id)                          -- 获取曲线
mesh:CurveID(curve)                     -- 曲线 ID
mesh:SelectNone()                       -- 取消选择

-- M_Point 方法
point.fPos:GetValue(frame)              -- 获取位置
point:SetCurvature(curvature, frame)    -- 设置曲率
point.fSelected                         -- 选择状态
```

### 骨骼操作

```lua
-- M_Skeleton 方法
skel:CountBones()                       -- 骨骼数
skel:Bone(id)                           -- 获取骨骼
skel:AddBone(frame)                     -- 添加骨骼（返回 M_Bone）
skel:BoneID(bone)                       -- 骨骼 ID

-- M_Bone 属性（直接赋值，不是 SetValue）
bone.fParent = -1                       -- 父骨骼 ID
bone.fLength = 2.0                      -- 长度
bone.fStrength = 1.0                    -- 强度
bone.fPos:Set(frame, LM.Vector2)        -- 位置
bone.fAngle:SetValue(frame, angle)      -- 角度（度）
bone:SetName(name)                      -- 名称
```

### 动画通道

```lua
-- AnimVal (数值)
channel:SetValue(frame, value)
channel:GetValue(frame)

-- AnimVec2 (2D 向量)
channel:SetValue(frame, LM.Vector2)
channel:GetValue(frame)

-- AnimColor (颜色)
channel:SetValue(frame, LM.ColorVector)  -- 或 rgb_color
channel:GetValue(frame)

-- AnimBool (布尔)
channel:SetValue(frame, bool)
channel:GetValue(frame)

-- AnimString (字符串)
channel:SetValue(frame, string)
channel:GetValue(frame)
```

### 图层操作

```lua
-- MohoLayer 方法
layer:Name()                            -- 图层名
layer:LayerType()                       -- 类型常量
layer:SetVisible(bool)                  -- 静态可见性
layer:IsVisible()                       -- 检查可见性
layer.fVisibility:SetValue(frame, bool) -- 动画可见性

-- MohoDoc 方法
doc:CountLayers()                       -- 图层数
doc:Layer(id)                           -- 获取图层
doc:LayerByName(name)                   -- 按名获取
doc:DuplicateLayer(layer)               -- 复制图层
doc:Width() / doc:Height()              -- 文档尺寸
```

---

## 图层类型常量

```lua
MOHO.LT_VECTOR   -- 矢量图层
MOHO.LT_IMAGE    -- 图像图层
MOHO.LT_BONE     -- 骨骼图层
MOHO.LT_GROUP    -- 分组图层
MOHO.LT_SWITCH   -- 切换图层
MOHO.LT_PARTICLE -- 粒子图层
MOHO.LT_NOTE     -- 注释图层
MOHO.LT_3D       -- 3D 图层
MOHO.LT_AUDIO    -- 音频图层
```

---

## ⚠️ 常见错误函数签名对照

| ❌ 错误调用 | ✅ 正确调用 | 来源 |
|------------|------------|------|
| `mesh:AddPoint(pos, pointID, segID, frame)` | `mesh:AddPoint(pos, curveID, segID, frame)` | pkg_moho.lua_pkg:258 |
| `bone.fLength:SetValue(frame, 2.0)` | `bone.fLength = 2.0` | pkg_moho.lua_pkg:550 |
| `moho.document:CreateNewLayer(type)` | `moho:CreateNewLayer(type)` | pkg_anime.lua_pkg:98 |
| `mesh:Point(0)` after `AddLonePoint` | `mesh:Point(n)` after all points added | 内存验证 |
| `mesh:WeldPoints(last, first)`不相邻 | `mesh:WeldPoints(duplicate, start)`相邻 | pkg_moho.lua_pkg:261 |

---

## 详细 API 参考

查看原始 lua_pkg 文件：

- `pkg_lm.lua_pkg` - 基础类型完整定义
- `pkg_lm_gui.lua_pkg` - GUI 组件完整定义
- `pkg_moho.lua_pkg` - Moho 核心对象完整定义
- `pkg_anime.lua_pkg` - ScriptInterface 完整定义