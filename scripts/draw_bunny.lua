-- Moho CLI Draw Script - Bunny

function MohoScript(moho)
	print("=== MohoScript Start ===")
	moho:FileNew()

	local mesh = moho:Mesh()
	if (mesh == nil) then
		print("ERROR: mesh is nil")
		moho:Quit()
		return
	end

	print("Mesh ready")

	-- Bunny body parts
	local v = LM.Vector2:new_local()

	-- Helper function
	local function DrawEllipse(cx, cy, rx, ry, numPts, r, g, b)
		local startPt = mesh:CountPoints()
		for i = 0, numPts - 1 do
			local angle = i * math.pi * 2 / numPts
			v.x = cx + rx * math.cos(angle)
			v.y = cy + ry * math.sin(angle)
			if i == 0 then mesh:AddLonePoint(v, 0)
			else mesh:AppendPoint(v, 0) end
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

	-- 白身、粉耳、黑眼、粉鼻
	DrawEllipse(0, -0.2, 0.35, 0.4, 16, 1.0, 1.0, 1.0)  -- 身体 (白)
	DrawEllipse(0, 0.35, 0.4, 0.38, 16, 1.0, 1.0, 1.0)  -- 头 (白)
	DrawEllipse(-0.15, 0.75, 0.1, 0.3, 12, 0.95, 0.85, 0.85)  -- 左耳 (粉)
	DrawEllipse(0.15, 0.75, 0.1, 0.3, 12, 0.95, 0.85, 0.85)  -- 右耳 (粉)
	DrawEllipse(-0.12, 0.4, 0.08, 0.08, 10, 0.1, 0.1, 0.1)  -- 左眼 (黑)
	DrawEllipse(0.12, 0.4, 0.08, 0.08, 10, 0.1, 0.1, 0.1)  -- 右眼 (黑)
	DrawEllipse(0, 0.28, 0.05, 0.04, 8, 0.95, 0.75, 0.75)  -- 鼻 (粉)
	print("Bunny drawn")

	print("Points: " .. mesh:CountPoints())
	print("Shapes: " .. mesh:CountShapes())

	moho:FileSaveAs("/tmp/bunny.moho")
	print("Saved: /tmp/bunny.moho")

	print("=== MohoScript Complete ===")
	moho:Quit()
end