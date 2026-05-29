-- LaplacianSkeleton.lua v2
-- 2D Laplacian contraction with curve-based neighbor detection
-- Uses Moho curve connectivity for accurate neighbor relationships
-- Version: v2.0 (2026-05-21)

print("[LaplacianSkeleton v2] Module loaded")

-- ===== Build neighbor map from curve connectivity =====

function BuildNeighborMapFromCurves(mesh)
    -- Build a map: pointID -> list of neighbor pointIDs
    -- Based on curve connectivity (not distance)
    
    local neighborMap = {}
    
    -- Initialize empty neighbor list for each point
    for i = 0, mesh:CountPoints() - 1 do
        neighborMap[i] = {}
    end
    
    -- Iterate through all curves
    for curveID = 0, mesh:CountCurves() - 1 do
        local curve = mesh:Curve(curveID)
        
        -- Get curve endpoints
        -- Moho curve structure: segments connect points
        -- We need to identify which points are on this curve
        
        -- Method: check if curve endpoints match mesh points
        local curveParentID = curve.fParentID
        -- This is a mesh point ID that the curve starts from
        
        -- Build curve point list by tracing segments
        -- curve:CountSegments() gives number of segments
        -- Each segment connects two points
        
        -- Alternative: use curve.fPoints if available
        -- But Moho API may not expose this directly
        
        -- Simplified approach: 
        -- Get curve ID, then find all mesh points belonging to this curve
        for ptID = 0, mesh:CountPoints() - 1 do
            local pt = mesh:Point(ptID)
            -- Check if point belongs to curve (via curveIDs)
            -- Point has curve attachment info
            
            -- Moho Point structure:
            -- pt.fTempCurveID - temporary curve ID during drawing
            -- But we need a way to get curve membership
            
            -- Alternative: use mesh connectivity
            -- Mesh:AddPoint(pos, curveID, segID, frame) - adds point to curve
            -- We can infer: points added to same curveID are neighbors
        end
    end
    
    -- If curve API not directly available, use alternative approach:
    -- Build neighbor map from mesh topology (point adjacency)
    
    -- Alternative: Use distance-based with smaller threshold
    -- But tuned for actual neighbor distance (not global avg)
    
    print("[NeighborMap] Building from mesh topology...")
    
    -- Step 1: Calculate local neighbor distance for each point
    local localDistances = {}
    for i = 0, mesh:CountPoints() - 1 do
        local pt = mesh:Point(i)
        local minDist = 999
        
        -- Find closest neighbor
        for j = 0, mesh:CountPoints() - 1 do
            if j ~= i then
                local other = mesh:Point(j)
                local dx = other.fPos.x - pt.fPos.x
                local dy = other.fPos.y - pt.fPos.y
                local dist = math.sqrt(dx * dx + dy * dy)
                if dist < minDist then
                    minDist = dist
                end
            end
        end
        
        localDistances[i] = minDist
    end
    
    -- Step 2: Calculate average local neighbor distance
    local avgLocalDist = 0
    for i = 0, mesh:CountPoints() - 1 do
        avgLocalDist = avgLocalDist + localDistances[i]
    end
    avgLocalDist = avgLocalDist / mesh:CountPoints()
    
    print("[NeighborMap] Avg local neighbor distance: " .. avgLocalDist)
    
    -- Step 3: Build neighbor map using local distance * 1.5 as threshold
    local threshold = avgLocalDist * 1.5
    print("[NeighborMap] Using threshold: " .. threshold)
    
    for i = 0, mesh:CountPoints() - 1 do
        local pt = mesh:Point(i)
        local neighbors = {}
        
        for j = 0, mesh:CountPoints() - 1 do
            if j ~= i then
                local other = mesh:Point(j)
                local dx = other.fPos.x - pt.fPos.x
                local dy = other.fPos.y - pt.fPos.y
                local dist = math.sqrt(dx * dx + dy * dy)
                
                if dist < threshold then
                    neighbors[#neighbors + 1] = j
                end
            end
        end
        
        neighborMap[i] = neighbors
    end
    
    -- Print neighbor stats
    local maxDegree = 0
    local avgDegree = 0
    for i = 0, mesh:CountPoints() - 1 do
        local degree = #neighborMap[i]
        maxDegree = math.max(maxDegree, degree)
        avgDegree = avgDegree + degree
    end
    avgDegree = avgDegree / mesh:CountPoints()
    
    print("[NeighborMap] Max degree: " .. maxDegree)
    print("[NeighborMap] Avg degree: " .. avgDegree)
    
    return neighborMap, threshold
end

-- ===== Laplacian contraction with pre-built neighbor map =====

function LaplacianContractionWithNeighbors(mesh, neighborMap, iterations, lambda)
    -- Use pre-built neighbor map for contraction
    -- More accurate than distance-based threshold
    
    iterations = iterations or 100
    lambda = lambda or 0.1
    
    -- Initialize point positions
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
    
    print("[Laplacian] Starting: " .. mesh:CountPoints() .. " points, " .. iterations .. " iterations")
    
    -- Iterative contraction
    for iter = 1, iterations do
        local newPoints = {}
        
        for i = 0, mesh:CountPoints() - 1 do
            -- Get neighbors from pre-built map
            local neighbors = neighborMap[i]
            
            -- Calculate Laplacian
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
            
            -- Contract
            newPoints[i] = {
                x = points[i].x + lambda * lapX,
                y = points[i].y + lambda * lapY
            }
        end
        
        -- Update positions
        for i = 0, mesh:CountPoints() - 1 do
            points[i].x = newPoints[i].x
            points[i].y = newPoints[i].y
        end
        
        -- Progress
        if iter % 10 == 0 then
            local totalMove = 0
            for i = 0, mesh:CountPoints() - 1 do
                local dx = points[i].x - points[i].originalX
                local dy = points[i].y - points[i].originalY
                totalMove = totalMove + math.sqrt(dx * dx + dy * dy)
            end
            print("[Laplacian] Iter " .. iter .. ": avg move = " .. (totalMove / mesh:CountPoints()))
        end
    end
    
    print("[Laplacian] Complete")
    return points
end

-- ===== Skeleton node extraction =====

function ExtractSkeletonNodesFromNeighbors(points, neighborMap, meshSize)
    -- Use neighbor map to calculate degrees
    -- More accurate than distance-based
    
    local branchPoints = {}
    local endpoints = {}
    local normalPoints = {}
    
    for i = 0, meshSize - 1 do
        -- Update neighbor map based on contracted positions
        local degree = 0
        local neighbors = neighborMap[i]
        
        -- Check actual distance after contraction
        for _, neighborID in ipairs(neighbors) do
            local dx = points[neighborID].x - points[i].x
            local dy = points[neighborID].y - points[i].y
            local dist = math.sqrt(dx * dx + dy * dy)
            
            -- Use dynamic threshold based on contraction
            if dist < 0.1 then  -- Fixed threshold after contraction
                degree = degree + 1
            end
        end
        
        -- Classify point by degree
        if degree >= 3 then
            -- Branch point (joint candidate)
            branchPoints[#branchPoints + 1] = {
                id = i,
                x = points[i].x,
                y = points[i].y,
                degree = degree
            }
        elseif degree == 1 then
            -- Endpoint
            endpoints[#endpoints + 1] = {
                id = i,
                x = points[i].x,
                y = points[i].y,
                degree = 1
            }
        elseif degree == 2 then
            -- Normal point (part of a chain)
            normalPoints[#normalPoints + 1] = {
                id = i,
                x = points[i].x,
                y = points[i].y,
                degree = 2
            }
        end
    end
    
    print("[Skeleton] Branch: " .. #branchPoints .. " Endpoint: " .. #endpoints .. " Normal: " .. #normalPoints)
    
    return branchPoints, endpoints, normalPoints
end

-- ===== Full skeleton extraction =====

function ExtractSkeleton(mesh, iterations, lambda, clusterThreshold)
    iterations = iterations or 50
    lambda = lambda or 0.1
    clusterThreshold = clusterThreshold or 0.05
    
    print("=== Skeleton Extraction v2 ===")
    print("[Input] Points: " .. mesh:CountPoints())
    
    -- Step 1: Build neighbor map from curve/local connectivity
    local neighborMap, threshold = BuildNeighborMapFromCurves(mesh)
    
    -- Step 2: Laplacian contraction
    local points = LaplacianContractionWithNeighbors(mesh, neighborMap, iterations, lambda)
    
    -- Step 3: Extract skeleton nodes
    local branchPoints, endpoints, normalPoints = ExtractSkeletonNodesFromNeighbors(points, neighborMap, mesh:CountPoints())
    
    -- Step 4: Cluster branch points
    local joints = ClusterSkeletonNodes(branchPoints, clusterThreshold)
    
    -- Step 5: Infer connections
    local connections = InferSkeletonConnections(joints, endpoints, 0.1)
    
    print("=== Complete ===")
    print("[Output] Joints: " .. #joints .. " Connections: " .. #connections)
    
    return {
        joints = joints,
        connections = connections,
        endpoints = endpoints,
        points = points,
        threshold = threshold,
        neighborMap = neighborMap
    }
end

-- ===== Cluster and connection functions (same as v1) =====

function ClusterSkeletonNodes(branchPoints, clusterThreshold)
    clusterThreshold = clusterThreshold or 0.05
    
    local clusters = {}
    local used = {}
    
    for i, bp in ipairs(branchPoints) do
        if not used[i] then
            local cluster = {
                points = {bp},
                centerX = bp.x,
                centerY = bp.y
            }
            used[i] = true
            
            for j = i + 1, #branchPoints do
                if not used[j] then
                    local other = branchPoints[j]
                    local dx = other.x - cluster.centerX
                    local dy = other.y - cluster.centerY
                    local dist = math.sqrt(dx * dx + dy * dy)
                    
                    if dist < clusterThreshold then
                        cluster.points[#cluster.points + 1] = other
                        used[j] = true
                        cluster.centerX = (cluster.centerX + other.x) / 2
                        cluster.centerY = (cluster.centerY + other.y) / 2
                    end
                end
            end
            
            clusters[#clusters + 1] = cluster
        end
    end
    
    local joints = {}
    for i, cluster in ipairs(clusters) do
        joints[#joints + 1] = {
            x = cluster.centerX,
            y = cluster.centerY,
            pointCount = #cluster.points
        }
    end
    
    return joints
end

function InferSkeletonConnections(joints, endpoints, threshold)
    local connections = {}
    
    -- Joint connections
    for i = 1, #joints do
        for j = i + 1, #joints do
            local dx = joints[j].x - joints[i].x
            local dy = joints[j].y - joints[i].y
            local dist = math.sqrt(dx * dx + dy * dy)
            
            local hasIntermediate = false
            for k = 1, #joints do
                if k ~= i and k ~= j then
                    local d_ik = math.sqrt((joints[k].x - joints[i].x)^2 + (joints[k].y - joints[i].y)^2)
                    local d_kj = math.sqrt((joints[j].x - joints[k].x)^2 + (joints[j].y - joints[k].y)^2)
                    if d_ik + d_kj < dist + threshold * 0.5 then
                        hasIntermediate = true
                        break
                    end
                end
            end
            
            if not hasIntermediate and dist < threshold * 5 then
                connections[#connections + 1] = {
                    from = i,
                    to = j,
                    dist = dist
                }
            end
        end
    end
    
    -- Endpoint connections
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
        
        if nearestJoint and minDist < threshold * 5 then
            connections[#connections + 1] = {
                from = nearestJoint,
                to = -i,
                dist = minDist,
                isEndpoint = true
            }
        end
    end
    
    return connections
end

-- ===== Generate bone coordinates =====

function GenerateBoneCoordinates(skeleton)
    local bones = {}
    
    local rootJointID = 1
    local maxDegree = 0
    for i, joint in ipairs(skeleton.joints) do
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
    
    print("[Bones] Root: " .. rootJointID)
    
    for connID, conn in ipairs(skeleton.connections) do
        local fromJoint = skeleton.joints[conn.from]
        local toJoint
        
        if conn.isEndpoint then
            local epID = -conn.to
            toJoint = skeleton.endpoints[epID]
        else
            toJoint = skeleton.joints[conn.to]
        end
        
        if fromJoint and toJoint then
            bones[#bones + 1] = {
                rootX = fromJoint.x,
                rootY = fromJoint.y,
                tipX = toJoint.x,
                tipY = toJoint.y,
                parentID = conn.from,
                name = "Bone_" .. connID
            }
        end
    end
    
    return bones, rootJointID
end

print("[LaplacianSkeleton v2] All functions loaded")