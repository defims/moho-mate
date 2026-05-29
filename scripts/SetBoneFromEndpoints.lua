-- SetBoneFromEndpoints.lua v7
-- 从根和尖两个坐标创建或调整骨骼
-- 优化：parentID 改为 parentBone，更直观
-- 修复：Skeleton() API 调用正确，IPC 模式同步 fPos

-- ============================================================
-- SetBoneFromEndpoints: 给定根和尖坐标创建或调整骨骼
-- ============================================================
-- 参数：
--   moho        - ScriptInterface
--   parentBone  - 父骨骼对象（nil = 根骨骼）
--   rootPos     - 骨骼根位置（全局坐标）
--   tipPos      - 骨骼尖位置（全局坐标）
--   targetBone  - 目标骨骼（Frame 0 时 nil=创建新骨骼，Frame > 0 时必须传入）
-- 
-- 自动获取：
--   frame → moho.frame
--   skel → boneLayer:Skeleton()
-- 
-- ⚠️ IPC 模式调用前必须用 SetCurFrame 切帧：
--   moho:SetCurFrame(targetFrame, false)  -- 同步 fPos
-- ============================================================

function SetBoneFromEndpoints(moho, parentBone, rootPos, tipPos, targetBone)
    -- ===== 从 moho 获取当前帧和骨骼层 =====
    local frame = moho.frame
    local boneLayer = moho:LayerAsBone(moho.layer)
    if boneLayer == nil then
        print("[SetBoneFromEndpoints] ERROR: No bone layer selected")
        print("  Call moho:SetSelLayer(skeletonLayer) first")
        return nil
    end
    local skel = boneLayer:Skeleton()
    
    -- ===== 获取父骨骼 ID =====
    local parentID = -1
    if parentBone then
        parentID = skel:BoneID(parentBone)
    end
    
    -- ===== 步骤 1：计算全局骨骼向量 =====
    local globalVec = LM.Vector2:new_local()
    globalVec:Set(tipPos.x - rootPos.x, tipPos.y - rootPos.y)
    
    -- ===== 步骤 2：计算全局角度 =====
    local globalAngle = math.atan2(globalVec.y, globalVec.x)
    
    -- 规范化到 [0, 2π)
    while globalAngle >= 2 * math.pi do globalAngle = globalAngle - 2 * math.pi end
    while globalAngle < 0 do globalAngle = globalAngle + 2 * math.pi end
    
    -- ===== 步骤 3：计算相对角度 =====
    local relativeAngle = globalAngle
    
    if parentID >= 0 then
        local parent = skel:Bone(parentID)
        if parent then
            -- 计算父骨骼全局角度
            local parentVec = LM.Vector2:new_local()
            parentVec:Set(parent.fLength, 0)
            
            local parentRoot = LM.Vector2:new_local()
            parentRoot:Set(0, 0)
            
            if frame > 0 then
                parent.fMovedMatrix:Transform(parentVec)
                parent.fMovedMatrix:Transform(parentRoot)
            else
                parent.fRestMatrix:Transform(parentVec)
                parent.fRestMatrix:Transform(parentRoot)
            end
            
            -- 父骨骼向量 = 尖 - 根
            parentVec.x = parentVec.x - parentRoot.x
            parentVec.y = parentVec.y - parentRoot.y
            
            local parentGlobalAngle = math.atan2(parentVec.y, parentVec.x)
            relativeAngle = globalAngle - parentGlobalAngle
        end
    end
    
    -- 规范化相对角度到 [0, 2π)
    while relativeAngle >= 2 * math.pi do relativeAngle = relativeAngle - 2 * math.pi end
    while relativeAngle < 0 do relativeAngle = relativeAngle + 2 * math.pi end
    
    -- ===== 步骤 4：计算长度（不变） =====
    local length = globalVec:Mag()
    
    -- ===== 步骤 5：转换根位置到父骨骼坐标系 =====
    local localRootPos = LM.Vector2:new_local()
    localRootPos:Set(rootPos.x, rootPos.y)
    
    if parentID >= 0 then
        local parent = skel:Bone(parentID)
        if parent then
            local invMatrix = LM.Matrix:new_local()
            if frame > 0 then
                invMatrix:Set(parent.fMovedMatrix)
            else
                invMatrix:Set(parent.fRestMatrix)
            end
            invMatrix:Invert()
            invMatrix:Transform(localRootPos)
        end
    end
    
    -- ===== 步骤 6：创建或调整骨骼 =====
    local bone = nil
    
    -- DEBUG 输出
    print("[SetBoneFromEndpoints] Frame=" .. frame)
    print("  Global angle: " .. (globalAngle * 180 / math.pi) .. "°")
    if parentID >= 0 then
        print("  Parent: " .. parentBone:Name())
        print("  Relative angle: " .. (relativeAngle * 180 / math.pi) .. "°")
    end
    print("  Length: " .. length)
    
    if frame == 0 then
        -- Frame 0：创建骨骼
        if targetBone then
            bone = targetBone
        else
            bone = skel:AddBone(frame)
        end
        
        bone.fParent = parentID
        bone.fAnimParent:SetValue(frame, parentID)
        bone.fLength = length
        bone.fStrength = 1.0
        
        -- 设置动画关键帧
        bone.fAnimPos:SetValue(frame, localRootPos)
        bone.fAnimAngle:SetValue(frame, relativeAngle)
        
        -- 设置当前帧值（fRestMatrix 需要）
        bone.fPos:Set(localRootPos.x, localRootPos.y)
        bone.fAngle = relativeAngle
        bone.fScale = 1.0
        
        print("  ✅ Created/Updated at frame 0")
        print("  fAnimPos: (" .. localRootPos.x .. ", " .. localRootPos.y .. ")")
        print("  fAnimAngle: " .. relativeAngle .. " rad")
        
    else
        -- Frame > 0：动画调整（必须提供 targetBone）
        if not targetBone then
            print("[SetBoneFromEndpoints] ERROR: targetBone required for frame > 0")
            return nil
        end
        bone = targetBone
        
        -- 计算缩放比例
        local scale = length / bone.fLength
        
        -- 设置动画关键帧
        bone.fAnimPos:SetValue(frame, localRootPos)
        bone.fAnimAngle:SetValue(frame, relativeAngle)
        bone.fAnimScale:SetValue(frame, scale)
        
        -- 设置当前帧值（fMovedMatrix 需要）
        bone.fPos:Set(localRootPos.x, localRootPos.y)
        bone.fAngle = relativeAngle
        bone.fScale = scale
        
        -- 添加关键帧标记
        moho:NewKeyframe(CHANNEL_BONE)
        moho:NewKeyframe(CHANNEL_BONE_T)
        
        print("  ✅ Animated at frame " .. frame)
        print("  Scale: " .. scale)
    end
    
    -- 更新骨骼矩阵
    if bone then
        skel:UpdateBoneMatrix(skel:BoneID(bone))
    end
    
    return bone
end

-- ============================================================
-- GetBoneEndpoints: 获取骨骼的根和尖全局坐标（当前帧）
-- ============================================================

function GetBoneEndpoints(moho, bone)
    local frame = moho.frame
    local rootPos = LM.Vector2:new_local()
    local tipPos = LM.Vector2:new_local()
    
    -- 骨骼局部坐标：根=(0,0)，尖=(length,0)
    rootPos:Set(0, 0)
    tipPos:Set(bone.fLength, 0)
    
    -- 使用对应帧的矩阵
    local matrix = frame > 0 and bone.fMovedMatrix or bone.fRestMatrix
    matrix:Transform(rootPos)
    matrix:Transform(tipPos)
    
    return rootPos, tipPos
end

-- ============================================================
-- ValidateBoneEndpoints: 验证骨骼是否匹配给定坐标（当前帧）
-- ============================================================

function ValidateBoneEndpoints(moho, bone, expectedRoot, expectedTip)
    local actualRoot, actualTip = GetBoneEndpoints(moho, bone)
    
    -- 计算误差
    local rootDiff = LM.Vector2:new_local()
    rootDiff:Set(actualRoot.x - expectedRoot.x, actualRoot.y - expectedRoot.y)
    local rootError = rootDiff:Mag()
    
    local tipDiff = LM.Vector2:new_local()
    tipDiff:Set(actualTip.x - expectedTip.x, actualTip.y - expectedTip.y)
    local tipError = tipDiff:Mag()
    
    -- 输出验证结果
    print("[Validate] " .. bone:Name())
    print("  Root: expected=(" .. expectedRoot.x .. "," .. expectedRoot.y .. ")")
    print("        actual=(" .. actualRoot.x .. "," .. actualRoot.y .. ")")
    print("        error=" .. string.format("%.4f", rootError))
    print("  Tip: expected=(" .. expectedTip.x .. "," .. expectedTip.y .. ")")
    print("       actual=(" .. actualTip.x .. "," .. actualTip.y .. ")")
    print("       error=" .. string.format("%.4f", tipError))
    
    local tolerance = 0.001
    local pass = rootError < tolerance and tipError < tolerance
    if pass then
        print("  ✅ PASS (tolerance=" .. tolerance .. ")")
    else
        print("  ❌ FAIL (tolerance=" .. tolerance .. ")")
    end
    
    return pass, rootError, tipError
end

print("[SetBoneFromEndpoints v7] Module loaded - parentBone + targetBone")