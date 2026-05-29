-- LaplacianSkeleton.lua
-- 2D 拉普拉斯收缩法计算骨架关节坐标
-- 输入：矢量层 Mesh
-- 输出：骨架关节坐标列表 + 连接关系
-- 版本：v1.0 (2026-05-21)

print("[LaplacianSkeleton] Module loaded (v1.0)")

-- ===== 辅助函数 =====

-- 获取点的邻接点（基于曲线连接）
local function GetNeighbors(mesh, pointID)
    local neighbors = {}
    
    -- 遍历所有曲线，找到包含该点的曲线
    for curveID = 0, mesh:CountCurves() - 1 do
        local curve = mesh:Curve(curveID)
        -- 检查曲线上的点
        for segID = 0, curve:CountSegments() - 1 do
            -- 曲线段连接两个点，需要从 Mesh 获取点 ID
            -- Moho API: 曲线段没有直接返回点 ID 的方法
            -- 简化方案：基于距离判断邻接关系
        end
    end
    
    -- 简化方案：基于距离阈值判断邻接（适用于密集点集）
    local threshold = 0.05  -- 阈值，可根据 mesh 密度调整
    local pt = mesh:Point(pointID)
    local px, py = pt.fPos.x, pt.fPos.y
    
    for i = 0, mesh:CountPoints() - 1 do
        if i ~= pointID then
            local other = mesh:Point(i)
            local dx = other.fPos.x - px
            local dy = other.fPos.y - py
            local dist = math.sqrt(dx * dx + dy * dy)
            if dist < threshold then
                neighbors[#neighbors + 1] = i
            end
        end
    end
    
    return neighbors
end

-- 计算拉普拉斯算子（平均邻接点位置 - 当前点位置）
local function ComputeLaplacian(mesh, pointID, neighbors)
    local pt = mesh:Point(pointID)
    local px, py = pt.fPos.x, pt.fPos.y
    
    if #neighbors == 0 then
        return 0, 0
    end
    
    -- 计算邻接点的平均位置
    local avgX, avgY = 0, 0
    for _, neighborID in ipairs(neighbors) do
        local neighbor = mesh:Point(neighborID)
        avgX = avgX + neighbor.fPos.x
        avgY = avgY + neighbor.fPos.y
    end
    avgX = avgX / #neighbors
    avgY = avgY / #neighbors
    
    -- Laplacian = 平均位置 - 当前位置
    return avgX - px, avgY - py
end

-- ===== 拉普拉斯收缩主函数 =====

function LaplacianContraction(mesh, iterations, lambda, threshold)
    -- 参数：
    -- mesh: 矢量层的 Mesh 对象
    -- iterations: 收缩迭代次数（默认 100）
    -- lambda: 收缩系数（默认 0.1）
    -- threshold: 邻接点距离阈值（默认自动计算）
    
    iterations = iterations or 100
    lambda = lambda or 0.1
    
    -- 自动计算阈值（基于 mesh 的平均点间距）
    if not threshold then
        local avgDist = 0
        local count = 0
        for i = 0, math.min(mesh:CountPoints() - 1, 100) do
            local pt = mesh:Point(i)
            for j = i + 1, math.min(mesh:CountPoints() - 1, 100) do
                local other = mesh:Point(j)
                local dx = other.fPos.x - pt.fPos.x
                local dy = other.fPos.y - pt.fPos.y
                avgDist = avgDist + math.sqrt(dx * dx + dy * dy)
                count = count + 1
            end
        end
        threshold = avgDist / count * 2  -- 阈值 = 2 * 平均间距
        print("[Laplacian] Auto threshold: " .. threshold)
    end
    
    -- 存储收缩后的点位置
    local points = {}
    for i = 0, mesh:CountPoints() - 1 do
        local pt = mesh:Point(i)
        points[i] = {
            x = pt.fPos.x,
            y = pt.fPos.y,
            originalX = pt.fPos.x,
            originalY = pt.fPos.y
        }
    end
    
    print("[Laplacian] Starting contraction: " .. mesh:CountPoints() .. " points, " .. iterations .. " iterations")
    
    -- 迭代收缩
    for iter = 1, iterations do
        -- 计算每个点的 Laplacian 并更新位置
        local newPoints = {}
        
        for i = 0, mesh:CountPoints() - 1 do
            -- 获取邻接点（使用收缩后的位置）
            local neighbors = {}
            for j = 0, mesh:CountPoints() - 1 do
                if j ~= i then
                    local dx = points[j].x - points[i].x
                    local dy = points[j].y - points[i].y
                    local dist = math.sqrt(dx * dx + dy * dy)
                    if dist < threshold then
                        neighbors[#neighbors + 1] = j
                    end
                end
            end
            
            -- 计算 Laplacian
            local lapX, lapY = 0, 0
            if #neighbors > 0 then
                local avgX, avgY = 0, 0
                for _, neighborID in ipairs(neighbors) do
                    avgX = avgX + points[neighborID].x
                    avgY = avgY + points[neighborID].y
                end
                avgX = avgX / #neighbors
                avgY = avgY / #neighbors
                lapX = avgX - points[i].x
                lapY = avgY - points[i].y
            end
            
            -- 收缩：newPos = oldPos + lambda * laplacian
            newPoints[i] = {
                x = points[i].x + lambda * lapX,
                y = points[i].y + lambda * lapY
            }
        end
        
        -- 更新点位置
        for i = 0, mesh:CountPoints() - 1 do
            points[i].x = newPoints[i].x
            points[i].y = newPoints[i].y
        end
        
        -- 每 10 次迭代输出进度
        if iter % 10 == 0 then
            -- 计算收缩比例
            local totalDist = 0
            for i = 0, mesh:CountPoints() - 1 do
                local dx = points[i].x - points[i].originalX
                local dy = points[i].y - points[i].originalY
                totalDist = totalDist + math.sqrt(dx * dx + dy * dy)
            end
            local avgMove = totalDist / mesh:CountPoints()
            print("[Laplacian] Iteration " .. iter .. ": avg movement = " .. avgMove)
        end
    end
    
    print("[Laplacian] Contraction complete")
    return points, threshold
end

-- ===== 骨架节点提取 =====

function ExtractSkeletonNodes(points, threshold, meshSize)
    -- 参数：
    -- points: 收缩后的点位置列表
    -- threshold: 邻接点距离阈值
    -- meshSize: 原始 mesh 点数
    
    -- 计算每个点的"分支度"（邻接点数量）
    local degrees = {}
    for i = 0, meshSize - 1 do
        local degree = 0
        for j = 0, meshSize - 1 do
            if j ~= i then
                local dx = points[j].x - points[i].x
                local dy = points[j].y - points[i].y
                local dist = math.sqrt(dx * dx + dy * dy)
                if dist < threshold then
                    degree = degree + 1
                end
            end
        end
        degrees[i] = degree
    end
    
    -- 提取骨架节点：
    -- 分支点（degree >= 3）：多条边的汇聚点，是关节
    -- 端点（degree == 1）：边缘点
    -- 连接点（degree == 2）：中间点，可能不需要
    
    local branchPoints = {}   -- 分支点（关节候选）
    local endpoints = {}      -- 端点
    
    for i = 0, meshSize - 1 do
        if degrees[i] >= 3 then
            branchPoints[#branchPoints + 1] = {
                id = i,
                x = points[i].x,
                y = points[i].y,
                degree = degrees[i]
            }
        elseif degrees[i] == 1 then
            endpoints[#endpoints + 1] = {
                id = i,
                x = points[i].x,
                y = points[i].y,
                degree = 1
            }
        end
    end
    
    print("[Skeleton] Branch points: " .. #branchPoints)
    print("[Skeleton] Endpoints: " .. #endpoints)
    
    return branchPoints, endpoints, degrees
end

-- ===== 骨架关节聚类（合并相近的节点）=====

function ClusterSkeletonNodes(branchPoints, clusterThreshold)
    -- 参数：
    -- branchPoints: 分支点列表
    -- clusterThreshold: 聚类距离阈值（默认 0.1）
    
    clusterThreshold = clusterThreshold or 0.1
    
    -- 简单聚类：合并距离小于阈值的节点
    local clusters = {}
    local used = {}
    
    for i, bp in ipairs(branchPoints) do
        if not used[i] then
            -- 创建新聚类
            local cluster = {
                points = {bp},
                centerX = bp.x,
                centerY = bp.y
            }
            used[i] = true
            
            -- 查找相近的点
            for j = i + 1, #branchPoints do
                if not used[j] then
                    local other = branchPoints[j]
                    local dx = other.x - cluster.centerX
                    local dy = other.y - cluster.centerY
                    local dist = math.sqrt(dx * dx + dy * dy)
                    if dist < clusterThreshold then
                        cluster.points[#cluster.points + 1] = other
                        used[j] = true
                        -- 更新聚类中心
                        cluster.centerX = (cluster.centerX + other.x) / 2
                        cluster.centerY = (cluster.centerY + other.y) / 2
                    end
                end
            end
            
            clusters[#clusters + 1] = cluster
        end
    end
    
    -- 提取聚类中心作为关节位置
    local joints = {}
    for i, cluster in ipairs(clusters) do
        joints[#joints + 1] = {
            x = cluster.centerX,
            y = cluster.centerY,
            pointCount = #cluster.points
        }
    end
    
    print("[Skeleton] Clusters: " .. #clusters .. " -> " .. #joints .. " joints")
    
    return joints, clusters
end

-- ===== 骨架连接关系推断 =====

function InferSkeletonConnections(joints, endpoints, threshold)
    -- 参数：
    -- joints: 关节列表
    -- endpoints: 端点列表
    -- threshold: 连接距离阈值
    
    -- 简化方案：基于距离推断连接
    -- 两个关节如果距离小于阈值，且没有其他关节在中间，则连接
    
    local connections = {}
    
    -- 关节之间的连接
    for i = 1, #joints do
        for j = i + 1, #joints do
            local dx = joints[j].x - joints[i].x
            local dy = joints[j].y - joints[i].y
            local dist = math.sqrt(dx * dx + dy * dy)
            
            -- 检查是否有其他关节在中间
            local hasIntermediate = false
            for k = 1, #joints do
                if k ~= i and k ~= j then
                    -- 检查 k 是否在 i-j 的线段上
                    local d_ik = math.sqrt((joints[k].x - joints[i].x)^2 + (joints[k].y - joints[i].y)^2)
                    local d_kj = math.sqrt((joints[j].x - joints[k].x)^2 + (joints[j].y - joints[k].y)^2)
                    if d_ik + d_kj < dist + threshold * 0.5 then
                        hasIntermediate = true
                        break
                    end
                end
            end
            
            if not hasIntermediate and dist < threshold * 10 then
                connections[#connections + 1] = {
                    from = i,
                    to = j,
                    dist = dist
                }
            end
        end
    end
    
    -- 端点连接到最近的关节
    for i, ep in ipairs(endpoints) do
        local minDist = 999
        local nearestJoint = nil
        for j, joint in ipairs(joints) do
            local dx = joint.x - ep.x
            local dy = joint.y - ep.y
            local dist = math.sqrt(dx * dx + dy * dy)
            if dist < minDist then
                minDist = dist
                nearestJoint = j
            end
        end
        if nearestJoint and minDist < threshold * 10 then
            connections[#connections + 1] = {
                from = nearestJoint,
                to = -i,  -- 端点用负数索引标识
                dist = minDist,
                isEndpoint = true
            }
        end
    end
    
    print("[Skeleton] Connections: " .. #connections)
    
    return connections
end

-- ===== 完整骨架提取流程 =====

function ExtractSkeleton(mesh, iterations, lambda, clusterThreshold)
    -- 参数：
    -- mesh: 矢量层的 Mesh 对象
    -- iterations: 收缩迭代次数（默认 100）
    -- lambda: 收缩系数（默认 0.1）
    -- clusterThreshold: 聚类距离阈值（默认 0.05）
    
    iterations = iterations or 100
    lambda = lambda or 0.1
    clusterThreshold = clusterThreshold or 0.05
    
    print("=== Skeleton Extraction Start ===")
    print("[Input] Mesh points: " .. mesh:CountPoints())
    
    -- Step 1: 拉普拉斯收缩
    local points, threshold = LaplacianContraction(mesh, iterations, lambda)
    
    -- Step 2: 提取骨架节点
    local branchPoints, endpoints, degrees = ExtractSkeletonNodes(points, threshold, mesh:CountPoints())
    
    -- Step 3: 聚类关节
    local joints, clusters = ClusterSkeletonNodes(branchPoints, clusterThreshold)
    
    -- Step 4: 推断连接关系
    local connections = InferSkeletonConnections(joints, endpoints, threshold)
    
    -- 输出结果
    print("=== Skeleton Extraction Complete ===")
    print("[Output] Joints: " .. #joints)
    print("[Output] Connections: " .. #connections)
    
    return {
        joints = joints,
        connections = connections,
        endpoints = endpoints,
        points = points,
        threshold = threshold
    }
end

-- ===== 生成骨骼坐标（用于 SetBoneFromEndpoints）=====

function GenerateBoneCoordinates(skeleton)
    -- 参数：
    -- skeleton: ExtractSkeleton 返回的骨架结构
    
    -- 根据连接关系生成骨骼的根尖坐标
    local bones = {}
    
    -- 找到根关节（degree 最大或位置最中心的关节）
    local rootJointID = 1
    local maxDegree = 0
    for i, joint in ipairs(skeleton.joints) do
        -- 计算连接数
        local degree = 0
        for _, conn in ipairs(skeleton.connections) do
            if conn.from == i or conn.to == i then
                degree = degree + 1
            end
        end
        if degree > maxDegree then
            maxDegree = degree
            rootJointID = i
        end
    end
    
    print("[Bones] Root joint: " .. rootJointID .. " (degree=" .. maxDegree .. ")")
    
    -- 为每个连接生成骨骼
    local root = LM.Vector2:new_local()
    local tip = LM.Vector2:new_local()
    
    for connID, conn in ipairs(skeleton.connections) do
        local fromJoint = skeleton.joints[conn.from]
        local toJoint
        
        if conn.isEndpoint then
            -- 端点
            local epID = -conn.to
            toJoint = skeleton.endpoints[epID]
        else
            toJoint = skeleton.joints[conn.to]
        end
        
        if fromJoint and toJoint then
            -- 创建骨骼坐标
            bones[#bones + 1] = {
                rootX = fromJoint.x,
                rootY = fromJoint.y,
                tipX = toJoint.x,
                tipY = toJoint.y,
                parentID = conn.from,  -- 父关节 ID
                name = "Bone_" .. connID
            }
        end
    end
    
    print("[Bones] Generated: " .. #bones .. " bone coordinates")
    
    return bones, rootJointID
end

-- ===== 输出 JSON 格式结果 =====

function OutputSkeletonJSON(skeleton, bones)
    -- 输出 JSON 格式的骨架数据（便于调试）
    
    local json = "{\n"
    json = json .. "  \"joints\": [\n"
    for i, joint in ipairs(skeleton.joints) do
        json = json .. "    {\"x\": " .. joint.x .. ", \"y\": " .. joint.y .. "},\n"
    end
    json = json .. "  ],\n"
    
    json = json .. "  \"bones\": [\n"
    for i, bone in ipairs(bones) do
        json = json .. "    {\"root\": [" .. bone.rootX .. ", " .. bone.rootY .. "], "
        json = json .. "\"tip\": [" .. bone.tipX .. ", " .. bone.tipY .. "]},\n"
    end
    json = json .. "  ]\n"
    json = json .. "}\n"
    
    return json
end

print("[LaplacianSkeleton] All functions loaded")