-- AutoBoneGenerator.lua
-- 自动骨骼生成器：从矢量层 Mesh 自动生成骨骼结构
-- 使用 Laplacian contraction + SetBoneFromEndpoints
-- 版本：v1.1 (2026-05-22)
-- 修正：不修改骨骼层位置，直接使用矢量层全局坐标

print("[AutoBoneGenerator] Module loaded (v1.0)")

-- ===== 加载依赖模块 =====

local home = os.getenv("HOME")

-- 加载 LaplacianSkeleton 模块
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/LaplacianSkeleton.lua")

-- 加载 SetBoneFromEndpoints 模块
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

-- ===== 核心函数 =====

-- 分析矢量层坐标范围
function AnalyzeMeshCoordinates(mesh)
    print("[Analyzer] Analyzing mesh coordinates...")
    
    local minX, maxX, minY, maxY = 999, -999, 999, -999
    local points = {}
    
    for i = 0, mesh:CountPoints() - 1 do
        local pt = mesh:Point(i)
        local x = pt.fPos.x
        local y = pt.fPos.y
        
        minX = math.min(minX, x)
        maxX = math.max(maxX, x)
        minY = math.min(minY, y)
        maxY = math.max(maxY, y)
        
        points[i] = {x = x, y = y}
    end
    
    local centerX = (minX + maxX) / 2
    local centerY = (minY + maxY) / 2
    local width = maxX - minX
    local height = maxY - minY
    
    print("[Analyzer] Range: X=[" .. minX .. ", " .. maxX .. "] Y=[" .. minY .. ", " .. maxY .. "]")
    print("[Analyzer] Center: (" .. centerX .. ", " .. centerY .. ")")
    print("[Analyzer] Size: " .. width .. " x " .. height)
    
    return {
        minX = minX,
        maxX = maxX,
        minY = minY,
        maxY = maxY,
        centerX = centerX,
        centerY = centerY,
        width = width,
        height = height,
        points = points
    }
end

-- 计算骨骼强度（基于原始点集分布）
function CalculateBoneStrength(joint, originalPoints, threshold)
    -- 统计距离骨骼关节 threshold 范围内的点数量
    local pointCount = 0
    local avgDist = 0
    
    for i = 0, #originalPoints do
        local pt = originalPoints[i]
        local dx = pt.x - joint.x
        local dy = pt.y - joint.y
        local dist = math.sqrt(dx * dx + dy * dy)
        
        if dist < threshold then
            pointCount = pointCount + 1
            avgDist = avgDist + dist
        end
    end
    
    if pointCount > 0 then
        avgDist = avgDist / pointCount
    end
    
    -- 骨骼强度 = 影响半径
    -- 基于平均距离 + 点密度调整
    local strength = avgDist * 1.2  -- 略大于平均距离
    
    -- 根据点密度调整
    if pointCount > 50 then
        strength = strength * 1.5  -- 高密度区域，扩大影响范围
    elseif pointCount < 10 then
        strength = strength * 0.8  -- 低密度区域，缩小影响范围
    end
    
    -- 限制强度范围
    strength = math.max(0.2, math.min(2.0, strength))
    
    return strength, pointCount
end

-- 构建骨骼树（确定父子关系）
function BuildBoneTree(skeleton)
    -- 根据连接关系构建骨骼树
    -- 策略：选择连接数最多的关节作为根
    
    local joints = skeleton.joints
    local connections = skeleton.connections
    
    -- 计算每个关节的连接数（degree）
    local degrees = {}
    for i = 1, #joints do
        degrees[i] = 0
        for _, conn in ipairs(connections) do
            if conn.from == i or conn.to == i then
                degrees[i] = degrees[i] + 1
            end
        end
    end
    
    -- 找到根关节（degree 最大）
    local rootJointID = 1
    local maxDegree = 0
    for i = 1, #joints do
        if degrees[i] > maxDegree then
            maxDegree = degrees[i]
            rootJointID = i
        end
    end
    
    print("[BoneTree] Root joint: " .. rootJointID .. " (degree=" .. maxDegree .. ")")
    
    -- 构建父子关系表
    -- BFS 从根关节遍历，确定每个关节的父关节
    local parents = {}
    local visited = {}
    local queue = {}
    
    -- 初始化：根关节无父关节
    parents[rootJointID] = nil
    visited[rootJointID] = true
    queue[#queue + 1] = rootJointID
    
    -- BFS 遍历
    while #queue > 0 do
        local current = queue[1]
        table.remove(queue, 1)
        
        -- 查找相邻关节
        for _, conn in ipairs(connections) do
            local neighbor = nil
            if conn.from == current and not conn.isEndpoint then
                neighbor = conn.to
            elseif conn.to == current and not conn.isEndpoint then
                neighbor = conn.from
            end
            
            if neighbor and not visited[neighbor] then
                parents[neighbor] = current
                visited[neighbor] = true
                queue[#queue + 1] = neighbor
            end
        end
    end
    
    -- 输出父子关系
    for i = 1, #joints do
        local parentID = parents[i]
        if parentID then
            print("[BoneTree] Joint " .. i .. " -> parent: " .. parentID)
        else
            print("[BoneTree] Joint " .. i .. " -> ROOT")
        end
    end
    
    return parents, rootJointID
end

-- 自动生成骨骼（核心函数）
function AutoGenerateBones(moho, mesh, boneLayer, iterations, lambda, clusterThreshold)
    -- 参数：
    -- moho: ScriptInterface 对象
    -- mesh: 矢量层的 Mesh 对象
    -- boneLayer: 骨骼层对象
    -- iterations: Laplacian 收缩迭代次数（默认 100）
    -- lambda: 收缩系数（默认 0.1）
    -- clusterThreshold: 聚类距离阈值（默认 0.05）
    
    iterations = iterations or 100
    lambda = lambda or 0.1
    clusterThreshold = clusterThreshold or 0.05
    
    print("=== AutoBoneGenerator Start ===")
    print("[Input] Mesh points: " .. mesh:CountPoints())
    
    -- Step 1: 分析矢量层坐标
    local meshInfo = AnalyzeMeshCoordinates(mesh)
    
    -- Step 2: Laplacian contraction
    local skeleton = ExtractSkeleton(mesh, iterations, lambda, clusterThreshold)
    
    -- 调试：检查骨架提取结果
    print("[DEBUG] Skeleton joints count: " .. #skeleton.joints)
    print("[DEBUG] Skeleton connections count: " .. #skeleton.connections)
    
    -- 如果 Laplacian 没找到关节，使用 QuickGenerateBones
    if #skeleton.joints == 0 then
        print("[WARN] Laplacian contraction found no joints, using QuickGenerateBones")
        
        -- 切换到 Frame 0
        moho:SetCurFrame(0, false)
        
        -- 获取 Skeleton 对象
        local boneLayerObj = moho:LayerAsBone(boneLayer)
        local skel = boneLayerObj:Skeleton()
        
        -- 使用简化版
        local cx, cy = meshInfo.centerX, meshInfo.centerY
        local h = meshInfo.height
        local w = meshInfo.width
        
        local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()
        
        -- Spine
        root:Set(cx, cy - h * 0.3)
        tip:Set(cx, cy + h * 0.3)
        local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)
        spine:SetName("Spine")
        spine.fStrength = h * 0.5
        
        print("[Quick] Created Spine at center")
        
        -- 更新骨骼矩阵
        skel:UpdateBoneMatrix(-1)
        
        -- 验证骨骼层位置
        local finalPos = boneLayer.fTranslation:GetValue(0)
        if math.abs(finalPos.x) < 0.001 and math.abs(finalPos.y) < 0.001 then
            print("[✅ PASS] Bone layer position unchanged (0, 0)")
        else
            print("[❌ FAIL] Bone layer position was modified!")
        end
        
        return {bones = {spine}, skeleton = skeleton, meshInfo = meshInfo}
    end
    
    -- Step 3: 构建骨骼树（确定父子关系）
    local parents, rootJointID = BuildBoneTree(skeleton)
    
    -- Step 4: 切换到 Frame 0
    moho:SetCurFrame(0, false)
    
    -- Step 5: 获取 Skeleton 对象
    local boneLayerObj = moho:LayerAsBone(boneLayer)
    local skel = boneLayerObj:Skeleton()
    
    -- Step 6: 创建骨骼（使用 SetBoneFromEndpoints）
    -- 直接使用矢量层全局坐标，不修改骨骼层位置
    local bones = {}
    local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()
    
    -- 首先创建根骨骼
    local rootJoint = skeleton.joints[rootJointID]
    
    -- 根骨骼：从根关节延伸一个短骨骼（作为参考）
    -- 使用全局坐标（Laplacian 收缩后的骨架坐标）
    root:Set(rootJoint.x, rootJoint.y)
    tip:Set(rootJoint.x, rootJoint.y + meshInfo.height * 0.3)
    
    local rootBone = SetBoneFromEndpoints(moho, nil, root, tip, nil)
    rootBone:SetName("Root")
    rootBone.fStrength = meshInfo.height * 0.5
    bones[rootJointID] = rootBone
    
    print("[Bones] Created Root bone at (" .. rootJoint.x .. ", " .. rootJoint.y .. ") strength=" .. rootBone.fStrength)
    
    -- 创建其他骨骼（从父骨骼延伸）
    for jointID = 1, #skeleton.joints do
        if jointID ~= rootJointID then
            local joint = skeleton.joints[jointID]
            local parentJointID = parents[jointID]
            
            if parentJointID then
                local parentJoint = skeleton.joints[parentJointID]
                
                -- 骨骼坐标：直接使用全局坐标（不需要减去中心）
                root:Set(parentJoint.x, parentJoint.y)
                tip:Set(joint.x, joint.y)
                
                -- 父骨骼对象
                local parentBone = bones[parentJointID]
                
                -- 创建骨骼
                local bone = SetBoneFromEndpoints(moho, parentBone, root, tip, nil)
                
                -- 设置骨骼名称
                local boneName = "Joint_" .. jointID
                bone:SetName(boneName)
                
                -- 计算骨骼强度
                local strength, pointCount = CalculateBoneStrength(joint, meshInfo.points, meshInfo.width * 0.1)
                bone.fStrength = strength
                
                bones[jointID] = bone
                
                print("[Bones] Created " .. boneName .. ": strength=" .. strength .. " (points=" .. pointCount .. ")")
            end
        end
    end
    
    -- Step 7: 更新骨骼矩阵
    skel:UpdateBoneMatrix(-1)
    print("[Skeleton] Bone matrices updated")
    
    -- Step 8: 验证骨骼
    print("=== Bone Validation ===")
    for i = 0, skel:CountBones() - 1 do
        local bone = skel:Bone(i)
        local pos0 = bone.fAnimPos:GetValue(0)
        local angle0 = bone.fAnimAngle:GetValue(0)
        print(string.format("[%d] %s: pos=(%.4f, %.4f) angle=%.4f rad parent=%d length=%.4f strength=%.4f",
            i, bone:Name(), pos0.x, pos0.y, angle0, bone.fParent, bone.fLength, bone.fStrength))
    end
    
    print("=== AutoBoneGenerator Complete ===")
    print("[Output] Total bones: " .. skel:CountBones())
    
    return {
        bones = bones,
        skeleton = skeleton,
        meshInfo = meshInfo,
        rootJointID = rootJointID
    }
end

-- 简化版：直接从骨架节点生成骨骼（不使用 Laplacian contraction）
function QuickGenerateBones(moho, mesh, boneLayer)
    -- 适用于简单角色：直接从 mesh 中心生成骨骼
    -- 不修改骨骼层位置，直接使用矢量层全局坐标
    
    print("=== QuickGenerateBones Start ===")
    
    -- 分析矢量层坐标
    local meshInfo = AnalyzeMeshCoordinates(mesh)
    
    -- 切换到 Frame 0
    moho:SetCurFrame(0, false)
    
    -- 获取 Skeleton 对象
    local boneLayerObj = moho:LayerAsBone(boneLayer)
    local skel = boneLayerObj:Skeleton()
    
    -- 创建根骨骼（Spine）- 使用全局坐标
    local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()
    
    -- 骨骼坐标：矢量层中心为基准的全局坐标
    local cx, cy = meshInfo.centerX, meshInfo.centerY
    local h = meshInfo.height
    local w = meshInfo.width
    
    -- Spine: 从中心向下延伸
    root:Set(cx, cy - h * 0.3)
    tip:Set(cx, cy + h * 0.3)
    local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)
    spine:SetName("Spine")
    spine.fStrength = h * 0.5
    
    print("[Quick] Created Spine at (" .. cx .. ", " .. cy .. ") length=" .. spine.fLength)
    
    -- 创建四肢骨骼（简化）
    local limbLength = h * 0.2
    local limbOffset = w * 0.15
    
    -- 右腿
    root:Set(cx + limbOffset, cy - h * 0.3)
    tip:Set(cx + limbOffset * 1.5, cy - h * 0.5)
    local rightLeg = SetBoneFromEndpoints(moho, spine, root, tip, nil)
    rightLeg:SetName("RightLeg")
    rightLeg.fStrength = h * 0.3
    
    -- 左腿
    root:Set(cx - limbOffset, cy - h * 0.3)
    tip:Set(cx - limbOffset * 1.5, cy - h * 0.5)
    local leftLeg = SetBoneFromEndpoints(moho, spine, root, tip, nil)
    leftLeg:SetName("LeftLeg")
    leftLeg.fStrength = h * 0.3
    
    -- 更新骨骼矩阵
    skel:UpdateBoneMatrix(-1)
    
    print("[Quick] Created " .. skel:CountBones() .. " bones")
    print("=== QuickGenerateBones Complete ===")
    
    return {
        bones = {spine, rightLeg, leftLeg},
        meshInfo = meshInfo
    }
end

-- 输出骨骼结构 JSON（调试用）
function OutputBoneStructureJSON(result)
    local json = "{\n"
    
    -- Mesh 信息
    json = json .. "  \"meshInfo\": {\n"
    json = json .. "    \"center\": [" .. result.meshInfo.centerX .. ", " .. result.meshInfo.centerY .. "],\n"
    json = json .. "    \"size\": [" .. result.meshInfo.width .. ", " .. result.meshInfo.height .. "]\n"
    json = json .. "  },\n"
    
    -- 骨骼信息
    json = json .. "  \"bones\": [\n"
    for i, bone in ipairs(result.bones) do
        json = json .. "    {\"name\": \"" .. bone:Name() .. "\", "
        json = json .. "\"strength\": " .. bone.fStrength .. ", "
        json = json .. "\"length\": " .. bone.fLength .. "},\n"
    end
    json = json .. "  ],\n"
    
    -- 关节信息
    json = json .. "  \"joints\": [\n"
    for i, joint in ipairs(result.skeleton.joints) do
        json = json .. "    {\"id\": " .. i .. ", \"x\": " .. joint.x .. ", \"y\": " .. joint.y .. "},\n"
    end
    json = json .. "  ]\n"
    
    json = json .. "}\n"
    
    return json
end

print("[AutoBoneGenerator] All functions loaded")