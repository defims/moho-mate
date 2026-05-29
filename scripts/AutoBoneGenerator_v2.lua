-- AutoBoneGenerator_v2.lua
-- Auto bone generator with improved neighbor detection
-- Uses LaplacianSkeleton_v2 (local neighbor distance)
-- Version: v2.0 (2026-05-21)

print("[AutoBoneGenerator v2] Module loaded")

local home = os.getenv("HOME")

-- Load modules
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/LaplacianSkeleton_v2.lua")
dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/SetBoneFromEndpoints.lua")

-- Analyze mesh coordinates
function AnalyzeMeshCoordinates(mesh)
    print("[Analyzer] Analyzing mesh...")
    
    local minX, maxX, minY, maxY = 999, -999, 999, -999
    local points = {}
    
    for i = 0, mesh:CountPoints() - 1 do
        local pt = mesh:Point(i)
        minX = math.min(minX, pt.fPos.x)
        maxX = math.max(maxX, pt.fPos.x)
        minY = math.min(minY, pt.fPos.y)
        maxY = math.max(maxY, pt.fPos.y)
        points[i] = {x = pt.fPos.x, y = pt.fPos.y}
    end
    
    local centerX = (minX + maxX) / 2
    local centerY = (minY + maxY) / 2
    local width = maxX - minX
    local height = maxY - minY
    
    print("[Analyzer] Center: (" .. centerX .. ", " .. centerY .. ")")
    print("[Analyzer] Size: " .. width .. " x " .. height)
    
    return {
        centerX = centerX,
        centerY = centerY,
        width = width,
        height = height,
        points = points
    }
end

-- Calculate bone strength
function CalculateBoneStrength(joint, originalPoints, threshold)
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
    
    local strength = avgDist * 1.2
    
    if pointCount > 50 then
        strength = strength * 1.5
    elseif pointCount < 10 then
        strength = strength * 0.8
    end
    
    strength = math.max(0.2, math.min(2.0, strength))
    
    return strength, pointCount
end

-- Build bone tree
function BuildBoneTree(skeleton)
    local joints = skeleton.joints
    local connections = skeleton.connections
    
    local degrees = {}
    for i = 1, #joints do
        degrees[i] = 0
        for _, conn in ipairs(connections) do
            if conn.from == i or conn.to == i then
                degrees[i] = degrees[i] + 1
            end
        end
    end
    
    local rootJointID = 1
    local maxDegree = 0
    for i = 1, #joints do
        if degrees[i] > maxDegree then
            maxDegree = degrees[i]
            rootJointID = i
        end
    end
    
    print("[BoneTree] Root: " .. rootJointID)
    
    local parents = {}
    local visited = {}
    local queue = {}
    
    parents[rootJointID] = nil
    visited[rootJointID] = true
    queue[#queue + 1] = rootJointID
    
    while #queue > 0 do
        local current = queue[1]
        table.remove(queue, 1)
        
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
    
    return parents, rootJointID
end

-- Auto generate bones
function AutoGenerateBones(moho, mesh, boneLayer, iterations, lambda, clusterThreshold)
    iterations = iterations or 50
    lambda = lambda or 0.1
    clusterThreshold = clusterThreshold or 0.05
    
    print("=== AutoGenerateBones v2 Start ===")
    
    -- Step 1: Analyze mesh
    local meshInfo = AnalyzeMeshCoordinates(mesh)
    
    -- Step 2: Extract skeleton (using improved neighbor detection)
    local skeleton = ExtractSkeleton(mesh, iterations, lambda, clusterThreshold)
    
    -- Step 3: Build bone tree
    local parents, rootJointID = BuildBoneTree(skeleton)
    
    -- Step 4: Set bone layer position
    local boneLayerPos = LM.Vector2:new_local()
    boneLayerPos:Set(meshInfo.centerX, meshInfo.centerY)
    boneLayer.fTranslation:SetValue(0, boneLayerPos)
    
    -- Step 5: Set frame 0
    moho:SetCurFrame(0, false)
    
    -- Step 6: Get skeleton object
    local boneLayerObj = moho:LayerAsBone(boneLayer)
    local skel = boneLayerObj:Skeleton()
    
    -- Step 7: Create bones
    local bones = {}
    local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()
    
    -- Create root bone
    local rootJoint = skeleton.joints[rootJointID]
    root:Set(0, 0)
    tip:Set(0, meshInfo.height * 0.3)
    
    local rootBone = SetBoneFromEndpoints(moho, nil, root, tip, nil)
    rootBone:SetName("Root")
    rootBone.fStrength = meshInfo.height * 0.5
    bones[rootJointID] = rootBone
    
    print("[Bones] Root created")
    
    -- Create other bones
    for jointID = 1, #skeleton.joints do
        if jointID ~= rootJointID then
            local joint = skeleton.joints[jointID]
            local parentJointID = parents[jointID]
            
            if parentJointID then
                local parentJoint = skeleton.joints[parentJointID]
                
                root:Set(parentJoint.x - meshInfo.centerX, parentJoint.y - meshInfo.centerY)
                tip:Set(joint.x - meshInfo.centerX, joint.y - meshInfo.centerY)
                
                local parentBone = bones[parentJointID]
                local bone = SetBoneFromEndpoints(moho, parentBone, root, tip, nil)
                
                bone:SetName("Joint_" .. jointID)
                
                local strength, pointCount = CalculateBoneStrength(joint, meshInfo.points, meshInfo.width * 0.1)
                bone.fStrength = strength
                
                bones[jointID] = bone
                print("[Bones] Joint_" .. jointID .. " created")
            end
        end
    end
    
    -- Step 8: Update matrices
    skel:UpdateBoneMatrix(-1)
    
    -- Step 9: Validate
    print("=== Validation ===")
    for i = 0, skel:CountBones() - 1 do
        local bone = skel:Bone(i)
        local pos0 = bone.fAnimPos:GetValue(0)
        local angle0 = bone.fAnimAngle:GetValue(0)
        print(string.format("[%d] %s: pos=(%.4f, %.4f) ang=%.4f parent=%d len=%.4f str=%.4f",
            i, bone:Name(), pos0.x, pos0.y, angle0, bone.fParent, bone.fLength, bone.fStrength))
    end
    
    print("=== AutoGenerateBones v2 Complete ===")
    
    return {
        bones = bones,
        skeleton = skeleton,
        meshInfo = meshInfo,
        rootJointID = rootJointID
    }
end

-- Quick generate (without Laplacian)
function QuickGenerateBones(moho, mesh, boneLayer)
    print("=== QuickGenerateBones ===")
    
    local meshInfo = AnalyzeMeshCoordinates(mesh)
    
    local boneLayerPos = LM.Vector2:new_local()
    boneLayerPos:Set(meshInfo.centerX, meshInfo.centerY)
    boneLayer.fTranslation:SetValue(0, boneLayerPos)
    
    moho:SetCurFrame(0, false)
    
    local boneLayerObj = moho:LayerAsBone(boneLayer)
    local skel = boneLayerObj:Skeleton()
    
    local root, tip = LM.Vector2:new_local(), LM.Vector2:new_local()
    
    root:Set(0, -meshInfo.height * 0.3)
    tip:Set(0, meshInfo.height * 0.3)
    local spine = SetBoneFromEndpoints(moho, nil, root, tip, nil)
    spine:SetName("Spine")
    spine.fStrength = meshInfo.height * 0.5
    
    local limbLength = meshInfo.height * 0.2
    local limbOffset = meshInfo.width * 0.15
    
    root:Set(limbOffset, -meshInfo.height * 0.3)
    tip:Set(limbOffset * 1.5, -meshInfo.height * 0.5)
    local rightLeg = SetBoneFromEndpoints(moho, spine, root, tip, nil)
    rightLeg:SetName("RightLeg")
    rightLeg.fStrength = meshInfo.height * 0.3
    
    root:Set(-limbOffset, -meshInfo.height * 0.3)
    tip:Set(-limbOffset * 1.5, -meshInfo.height * 0.5)
    local leftLeg = SetBoneFromEndpoints(moho, spine, root, tip, nil)
    leftLeg:SetName("LeftLeg")
    leftLeg.fStrength = meshInfo.height * 0.3
    
    skel:UpdateBoneMatrix(-1)
    
    print("[Quick] " .. skel:CountBones() .. " bones created")
    
    return {
        bones = {spine, rightLeg, leftLeg},
        meshInfo = meshInfo
    }
end

print("[AutoBoneGenerator v2] All functions loaded")