-- BoneReparent.lua
-- 从 lm_reparent_bone.lua 提取的骨骼父子关系设置核心函数
-- 用法：dofile(home .. "/.openclaw/workspace/skills/moho-mate/scripts/BoneReparent.lua")
--       ReparentBone(moho, skel, bone, newParentID, frame)

-- ============================================================
-- ReparentBone: 设置骨骼的父子关系并自动转换坐标
-- ============================================================
-- 参数：
--   moho: Moho ScriptInterface 对象
--   skel: Skeleton 对象
--   bone: 要设置父级的骨骼（M_Bone 对象）
--   newParentID: 新父骨骼的 ID（-1 表示无父级/根骨骼）
--   frame: 帧号（通常为 0）
-- ============================================================
-- 返回：
--   成功返回 true，失败返回 false
-- ============================================================
-- 核心逻辑（来自 lm_reparent_bone.lua）：
--   1. 用当前骨骼矩阵变换到全局坐标
--   2. 用新父骨骼逆矩阵变换回局部坐标
--   3. 计算相对角度
--   4. 更新 fParent 和 fAnimParent
-- ============================================================

function ReparentBone(moho, skel, bone, newParentID, frame)
    -- 检查参数
    if skel == nil or bone == nil then
        print("[ReparentBone] ERROR: skel or bone is nil")
        return false
    end
    
    -- 检查循环依赖（不能将骨骼设为其子孙的子级）
    local boneID = skel:BoneID(bone)
    if newParentID >= 0 and skel:IsBoneChild(newParentID, boneID) then
        print("[ReparentBone] ERROR: Cannot parent to descendant")
        return false
    end
    
    -- ===== 步骤 1：获取当前全局坐标 =====
    -- 骨骼起点（v1）和末端（v2）在骨骼自身坐标系中
    local v1 = LM.Vector2:new_local()
    local v2 = LM.Vector2:new_local()
    
    v1:Set(0, 0)  -- 骨骼起点
    
    if bone:IsZeroLength() then
        v2:Set(0.1, 0)  -- 零长度骨骼用默认方向
    else
        v2:Set(bone.fLength, 0)  -- 骨骼末端
    end
    
    -- 用骨骼矩阵变换到全局坐标
    if frame > 0 then
        bone.fMovedMatrix:Transform(v1)
        bone.fMovedMatrix:Transform(v2)
    else
        bone.fRestMatrix:Transform(v1)
        bone.fRestMatrix:Transform(v2)
    end
    
    -- ===== 步骤 2：变换到新父骨骼坐标系 =====
    if newParentID >= 0 then
        local invMatrix = LM.Matrix:new_local()
        local newParent = skel:Bone(newParentID)
        
        if newParent == nil then
            print("[ReparentBone] ERROR: Parent bone not found")
            return false
        end
        
        -- 获取新父骨骼矩阵并求逆
        if frame > 0 then
            invMatrix:Set(newParent.fMovedMatrix)
        else
            invMatrix:Set(newParent.fRestMatrix)
        end
        invMatrix:Invert()
        
        -- 变换到新父骨骼坐标系
        invMatrix:Transform(v1)
        invMatrix:Transform(v2)
    end
    
    -- ===== 步骤 3：设置新位置 =====
    bone.fAnimPos:SetValue(frame, v1)
    
    -- ===== 步骤 4：计算并设置新角度 =====
    -- 骨骼方向向量
    local dirVec = v2 - v1
    local angle = math.atan(dirVec.y, dirVec.x)
    
    -- 规范化角度到 [0, 2π]
    while angle > 2 * math.pi do
        angle = angle - 2 * math.pi
    end
    while angle < 0 do
        angle = angle + 2 * math.pi
    end
    
    bone.fAnimAngle:SetValue(frame, angle)
    
    -- ===== 步骤 5：更新父子关系 =====
    -- 保存旧的父级（用于 fAnimParent）
    local oldParent = bone.fParent
    
    -- 设置新父级
    bone.fParent = newParentID
    bone.fAnimParent:SetValue(frame, newParentID)
    
    -- ===== 步骤 6：更新骨骼矩阵 =====
    skel:UpdateBoneMatrix(boneID)
    
    print("[ReparentBone] " .. bone:Name() .. " parent changed: " .. oldParent .. " -> " .. newParentID)
    print("[ReparentBone] New fAnimPos: (" .. v1.x .. ", " .. v1.y .. ")")
    print("[ReparentBone] New fAnimAngle: " .. angle .. " rad")
    
    return true
end

-- ============================================================
-- GetBoneGlobalPos: 获取骨骼的全局位置（起点）
-- ============================================================
-- 参数：
--   skel: Skeleton 对象
--   bone: 骨骼对象
--   frame: 帧号
-- ============================================================
-- 返回：
--   LM.Vector2 全局位置
-- ============================================================

function GetBoneGlobalPos(skel, bone, frame)
    local v = LM.Vector2:new_local()
    v:Set(0, 0)  -- 骨骼起点
    
    if frame > 0 then
        bone.fMovedMatrix:Transform(v)
    else
        bone.fRestMatrix:Transform(v)
    end
    
    return v
end

-- ============================================================
-- GetBoneGlobalAngle: 获取骨骼的全局角度（弧度）
-- ============================================================
-- 参数：
--   skel: Skeleton 对象
--   bone: 骨骼对象
--   frame: 帧号
-- ============================================================
-- 返回：
--   全局角度（弧度）
-- ============================================================

function GetBoneGlobalAngle(skel, bone, frame)
    local v1 = LM.Vector2:new_local()
    local v2 = LM.Vector2:new_local()
    
    v1:Set(0, 0)
    
    if bone:IsZeroLength() then
        v2:Set(0.1, 0)
    else
        v2:Set(bone.fLength, 0)
    end
    
    if frame > 0 then
        bone.fMovedMatrix:Transform(v1)
        bone.fMovedMatrix:Transform(v2)
    else
        bone.fRestMatrix:Transform(v1)
        bone.fRestMatrix:Transform(v2)
    end
    
    local dirVec = v2 - v1
    return math.atan(dirVec.y, dirVec.x)
end

-- ============================================================
-- TransformToLocalPos: 将全局位置变换到父骨骼坐标系
-- ============================================================
-- 参数：
--   globalPos: LM.Vector2 全局位置
--   parentBone: 父骨骼对象（nil 表示变换到图层坐标系）
--   frame: 帧号
-- ============================================================
-- 返回：
--   LM.Vector2 局部位置
-- ============================================================

function TransformToLocalPos(globalPos, parentBone, frame)
    local localPos = LM.Vector2:new_local()
    localPos:Set(globalPos.x, globalPos.y)
    
    if parentBone ~= nil then
        local invMatrix = LM.Matrix:new_local()
        
        if frame > 0 then
            invMatrix:Set(parentBone.fMovedMatrix)
        else
            invMatrix:Set(parentBone.fRestMatrix)
        end
        invMatrix:Invert()
        invMatrix:Transform(localPos)
    end
    
    return localPos
end

-- ============================================================
-- TransformToGlobalPos: 将局部位置变换到全局坐标系
-- ============================================================
-- 参数：
--   localPos: LM.Vector2 局部位置（父骨骼坐标系）
--   parentBone: 父骨骼对象
--   frame: 帧号
-- ============================================================
-- 返回：
--   LM.Vector2 全局位置
-- ============================================================

function TransformToGlobalPos(localPos, parentBone, frame)
    local globalPos = LM.Vector2:new_local()
    globalPos:Set(localPos.x, localPos.y)
    
    if parentBone ~= nil then
        local matrix = LM.Matrix:new_local()
        
        if frame > 0 then
            matrix:Set(parentBone.fMovedMatrix)
        else
            matrix:Set(parentBone.fRestMatrix)
        end
        matrix:Transform(globalPos)
    end
    
    return globalPos
end

print("[BoneReparent] Module loaded")
print("  Functions: ReparentBone, GetBoneGlobalPos, GetBoneGlobalAngle")
print("  TransformToLocalPos, TransformToGlobalPos")