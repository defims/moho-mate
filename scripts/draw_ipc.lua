-- draw_ipc.lua - IPC 模式绘制形状
-- 用法: dofile 此脚本，调用 draw_shape(shape)
--
-- 注意：draw 只绘制，不保存
-- 用户需手动保存（Cmd+S）

function draw_shape(shape)
    local moho = _G.moho
    if not moho then
        print("✗ moho 对象不存在")
        return false
    end
    
    local doc = moho.document
    if not doc then
        print("✗ document 不存在")
        return false
    end
    
    local current_path = doc:Path()
    print("当前文档: " .. (current_path or "Untitled"))
    print("⚠️ draw 只绘制，不保存。请手动 Cmd+S 保存")
    
    -- 创建图层
    local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)
    if not layer then
        print("✗ 无法创建图层")
        return false
    end
    print("✓ 图层已创建")
    
    if shape == "circle" then
        layer:SetName("CircleLayer")
        moho:SetSelLayer(layer)
        
        -- DrawingMesh 需要先选中图层
        local mesh = moho:DrawingMesh()
        print("mesh: " .. tostring(mesh))
        
        if not mesh then
            print("✗ 无法获取 mesh，尝试 LayerAsVector")
            local vlayer = moho:LayerAsVector(layer)
            print("vlayer: " .. tostring(vlayer))
            if vlayer then
                mesh = vlayer:Mesh()
                print("mesh from vlayer: " .. tostring(mesh))
            end
        end
        
        if not mesh then
            print("✗ 无法获取 mesh")
            return false
        end
        local v = LM.Vector2:new_local()
        
        for i = 0, 15 do
            local angle = i * math.pi * 2 / 16
            v:Set(0.3 * math.cos(angle), 0.3 * math.sin(angle))
            if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end
        end
        
        mesh:WeldPoints(15, 0, 0)
        -- WeldPoints 后点数变成15，不能循环到15
        mesh:SelectNone()
        for i = 0, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end
        
        local shapeID = moho:CreateShape(true, false, 0)
        if shapeID >= 0 then
            local s = mesh:Shape(shapeID)
            s.fHasFill = true
            s.fMyStyle.fDefineFillCol = true
            local col = LM.ColorVector:new_local()
            col:Set(0.4, 0.6, 1.0, 1.0)
            s.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())
        end
        print("✓ circle 已绘制")
        
    elseif shape == "bunny" then
        layer:SetName("BunnyLayer")
        moho:SetSelLayer(layer)
        local mesh = moho:DrawingMesh()
        local v = LM.Vector2:new_local()
        
        -- 身体
        for i = 0, 15 do
            local angle = i * math.pi * 2 / 16
            v:Set(0.3 * math.cos(angle), 0.2 * math.sin(angle))
            if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end
        end
        mesh:WeldPoints(15, 0, 0)
        mesh:SelectNone()
        -- WeldPoints 后点数减少
        for i = 0, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end
        moho:CreateShape(true, false, 0)
        
        -- 头部
        for i = 0, 15 do
            local angle = i * math.pi * 2 / 16
            v:Set(0.15 + 0.15 * math.cos(angle), 0.35 + 0.12 * math.sin(angle))
            if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end
        end
        mesh:WeldPoints(mesh:CountPoints() - 1, mesh:CountPoints() - 16, 0)
        mesh:SelectNone()
        local start = mesh:CountPoints() - 16
        for i = start, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end
        moho:CreateShape(true, false, 0)
        print("✓ bunny 已绘制")
        
    elseif shape == "puppy" then
        layer:SetName("PuppyLayer")
        moho:SetSelLayer(layer)
        local mesh = moho:DrawingMesh()
        local v = LM.Vector2:new_local()
        
        -- 身体（金色）
        for i = 0, 15 do
            local angle = i * math.pi * 2 / 16
            v:Set(0.35 * math.cos(angle), 0.25 * math.sin(angle))
            if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end
        end
        mesh:WeldPoints(15, 0, 0)
        mesh:SelectNone()
        -- WeldPoints 后点数减少
        for i = 0, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end
        
        local shapeID = moho:CreateShape(true, false, 0)
        if shapeID >= 0 then
            local s = mesh:Shape(shapeID)
            s.fHasFill = true
            s.fMyStyle.fDefineFillCol = true
            local col = LM.ColorVector:new_local()
            col:Set(0.8, 0.6, 0.2, 1.0)
            s.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())
        end
        
        -- 头部
        for i = 0, 15 do
            local angle = i * math.pi * 2 / 16
            v:Set(0.2 + 0.15 * math.cos(angle), 0.4 + 0.12 * math.sin(angle))
            if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end
        end
        mesh:WeldPoints(mesh:CountPoints() - 1, mesh:CountPoints() - 16, 0)
        mesh:SelectNone()
        local start = mesh:CountPoints() - 16
        for i = start, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end
        moho:CreateShape(true, false, 0)
        print("✓ puppy 已绘制")
    end
    
    print("✓ 绘制完成，请手动保存")
    return true
end

print("✓ draw_ipc.lua 已加载")
