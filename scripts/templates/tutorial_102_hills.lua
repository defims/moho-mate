-- Tutorial 1.02: Drawing Simple Shapes - Hills
-- 山丘形状 + 单色填充（IPC 模式）

-- 使用方法：
-- moho-mate start
-- moho-mate call -f tutorial_102_hills.lua
-- moho-mate quit

local mesh = moho:Mesh()
if mesh == nil then
    print("ERROR: mesh is nil")
    return
end

local v = LM.Vector2:new_local()
local frame = 0

-- 山丘形状：7点 + 闭合
-- 点顺序：左下 → 右下 → 右上 → 中右凹陷 → 中央凸起 → 中左凹陷 → 左上 → 闭合
local n = mesh:CountPoints()

v:Set(-2, 0)       -- 左下
mesh:AddLonePoint(v, frame)

v:Set(2, 0)        -- 右下
mesh:AppendPoint(v, frame)

v:Set(2, 0.5)      -- 右上
mesh:AppendPoint(v, frame)

v:Set(1.2, 0.3)    -- 中右凹陷
mesh:AppendPoint(v, frame)

v:Set(0, 1.2)      -- 中央凸起（最高点）
mesh:AppendPoint(v, frame)

v:Set(-1.2, 0.3)   -- 中左凹陷
mesh:AppendPoint(v, frame)

v:Set(-2, 0.5)     -- 左上
mesh:AppendPoint(v, frame)

v:Set(-2, 0)       -- 回起点（闭合）
mesh:AppendPoint(v, frame)

-- Weld 闭合（8点 Weld 成 7点闭合曲线）
mesh:WeldPoints(n + 7, n, frame)

-- 设置曲率（平滑）
for i = n, n + 6 do
    mesh:Point(i):SetCurvature(MOHO.SMOOTH, frame)
end

-- 手动选择点（SelectConnected 不稳定）
mesh:SelectNone()
for i = n, n + 6 do
    mesh:Point(i).fSelected = true
end

-- 创建形状
local shapeID = moho:CreateShape(true, false, frame)
print("Shape ID: " .. shapeID)

if shapeID >= 0 then
    local shape = mesh:Shape(shapeID)
    
    -- 填充设置（正确 API）
    shape.fHasFill = true
    shape.fMyStyle.fDefineFillCol = true
    local col = LM.ColorVector:new_local()
    col:Set(0.2, 0.6, 0.3, 1.0)  -- RGBA (0-1)
    shape.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())
    
    -- 线宽设置（正确 API）
    shape.fMyStyle.fDefineLineWidth = true
    shape.fMyStyle.fLineWidth = 1.0  -- real 类型
    
    -- 边框颜色
    shape.fHasOutline = true
    shape.fMyStyle.fDefineLineCol = true
    col:Set(0.1, 0.3, 0.1, 1.0)
    shape.fMyStyle.fLineCol:SetValue(0, col:AsColorStruct())
end

-- 重命名图层
moho.layer:SetName("Hills")

print("Points: " .. mesh:CountPoints())
print("Shapes: " .. mesh:CountShapes())

-- 保存
local outputPath = "/tmp/Tutorial_1_02_Hills.moho"
moho:FileSaveAs(outputPath)
print("Saved to: " .. outputPath)

-- IPC 模式：不调用 moho:Quit()