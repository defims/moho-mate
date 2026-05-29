-- Moho CLI Draw Script - Circle

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

	-- Draw circle
	local v = LM.Vector2:new_local()
	local numPts = 16
	local cx = 0
	local cy = 0
	local rx = 0.3
	local ry = 0.3

	for i = 0, numPts - 1 do
		local angle = i * math.pi * 2 / numPts
		v.x = cx + rx * math.cos(angle)
		v.y = cy + ry * math.sin(angle)
		if i == 0 then
			mesh:AddLonePoint(v, 0)
		else
			mesh:AppendPoint(v, 0)
		end
	end

	mesh:WeldPoints(mesh:CountPoints() - 1, 0, 0)
	mesh:SelectNone()
	for i = 0, mesh:CountPoints() - 1 do
		mesh:Point(i).fSelected = true
	end

	local shapeID = moho:CreateShape(true, false, 0)
	if shapeID >= 0 then
		local shape = mesh:Shape(shapeID)
		shape.fHasFill = true
		shape.fMyStyle.fDefineFillCol = true
		local col = LM.ColorVector:new_local()
		col:Set(0.4, 0.6, 1.0, 1.0)  -- 蓝色
		shape.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())
	end
	print("Circle drawn")

	print("Points: " .. mesh:CountPoints())
	print("Shapes: " .. mesh:CountShapes())

	moho:FileSaveAs("/tmp/circle.moho")
	print("Saved: /tmp/circle.moho")

	print("=== MohoScript Complete ===")
	moho:Quit()
end