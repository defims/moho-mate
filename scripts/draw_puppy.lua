-- Moho CLI Draw Script - Puppy

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

	-- Puppy body parts
	local v = LM.Vector2:new_local()

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

	-- 金黄身、棕耳、黑眼
	DrawEllipse(0, -0.15, 0.4, 0.35, 16, 0.85, 0.65, 0.2)  -- 身体 (金黄)
	DrawEllipse(0.35, 0.2, 0.3, 0.28, 16, 0.85, 0.65, 0.2)  -- 头 (金黄)
	DrawEllipse(0.2, 0.35, 0.12, 0.22, 12, 0.5, 0.35, 0.15)  -- 左耳 (棕)
	DrawEllipse(0.5, 0.35, 0.12, 0.22, 12, 0.5, 0.35, 0.15)  -- 右耳 (棕)
	DrawEllipse(0.25, 0.25, 0.06, 0.06, 10, 0.1, 0.1, 0.1)  -- 左眼 (黑)
	DrawEllipse(0.45, 0.25, 0.06, 0.06, 10, 0.1, 0.1, 0.1)  -- 右眼 (黑)
	DrawEllipse(0.35, 0.12, 0.05, 0.04, 8, 0.15, 0.15, 0.15)  -- 鼻 (黑)
	print("Puppy drawn")

	print("Points: " .. mesh:CountPoints())
	print("Shapes: " .. mesh:CountShapes())

	moho:FileSaveAs("/tmp/puppy.moho")
	print("Saved: /tmp/puppy.moho")

	print("=== MohoScript Complete ===")
	moho:Quit()
end