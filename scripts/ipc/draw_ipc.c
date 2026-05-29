// draw_ipc.c - draw 命令 IPC 模式实现
// 将此代码插入 moho-mate.c 的 draw 函数位置

// ========== draw（IPC 模式）==========

static int cmd_draw(int argc, char **argv) {
    char *shape = argc > 1 ? argv[1] : "circle";
    char *output = argc > 2 ? argv[2] : NULL;
    
    // 检查支持的形状
    if (strcmp(shape, "circle") != 0 && strcmp(shape, "bunny") != 0 && strcmp(shape, "puppy") != 0) {
        fprintf(stderr, "✗ 未知形状: %s\n", shape);
        fprintf(stderr, "可用形状: circle, bunny, puppy\n");
        return 1;
    }
    
    printf("▶ 绘制形状: %s\n", shape);
    
    // 默认输出路径
    char output_path[512];
    if (output) {
        snprintf(output_path, sizeof(output_path), "%s", output);
    } else {
        snprintf(output_path, sizeof(output_path), "/tmp/%s.moho", shape);
    }
    printf("  输出: %s\n", output_path);
    
    auto_start_ipc();
    
    // 加载 IPCFileSaveAs 模块
    char load_module[512];
    snprintf(load_module, sizeof(load_module),
        "local home = os.getenv('HOME')\n"
        "dofile(home .. '/.openclaw/workspace/skills/moho-mate/scripts/IPCFileSaveAs.lua')\n");
    ipc_send_multiline(load_module);
    
    // 构建绘制 Lua 代码（IPC 模式）
    char lua_cmd[2048];
    
    if (strcmp(shape, "circle") == 0) {
        snprintf(lua_cmd, sizeof(lua_cmd),
            "moho:FileNew()\n"
            "local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)\n"
            "layer:SetName('CircleLayer')\n"
            "moho:SetSelLayer(layer)\n"
            "local mesh = moho:DrawingMesh()\n"
            "local v = LM.Vector2:new_local()\n"
            "for i = 0, 15 do\n"
            "  local angle = i * math.pi * 2 / 16\n"
            "  v:Set(0.3 * math.cos(angle), 0.3 * math.sin(angle))\n"
            "  if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end\n"
            "end\n"
            "mesh:WeldPoints(15, 0, 0)\n"
            "mesh:SelectNone()\n"
            "for i = 0, 15 do mesh:Point(i).fSelected = true end\n"
            "local shapeID = moho:CreateShape(true, false, 0)\n"
            "if shapeID >= 0 then\n"
            "  local s = mesh:Shape(shapeID)\n"
            "  s.fHasFill = true\n"
            "  s.fMyStyle.fDefineFillCol = true\n"
            "  local col = LM.ColorVector:new_local()\n"
            "  col:Set(0.4, 0.6, 1.0, 1.0)\n"
            "  s.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())\n"
            "end\n"
            "IPCFileSaveAs(moho, '%s')\n"
            "print('✓ 圆形已保存')\n",
            output_path);
    } else if (strcmp(shape, "bunny") == 0) {
        snprintf(lua_cmd, sizeof(lua_cmd),
            "moho:FileNew()\n"
            "local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)\n"
            "layer:SetName('BunnyLayer')\n"
            "moho:SetSelLayer(layer)\n"
            "local mesh = moho:DrawingMesh()\n"
            "local v = LM.Vector2:new_local()\n"
            "-- 身体\n"
            "for i = 0, 15 do\n"
            "  local angle = i * math.pi * 2 / 16\n"
            "  v:Set(0.3 * math.cos(angle), 0.2 * math.sin(angle))\n"
            "  if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end\n"
            "end\n"
            "mesh:WeldPoints(15, 0, 0)\n"
            "mesh:SelectNone()\n"
            "for i = 0, 15 do mesh:Point(i).fSelected = true end\n"
            "moho:CreateShape(true, false, 0)\n"
            "-- 头部\n"
            "for i = 0, 15 do\n"
            "  local angle = i * math.pi * 2 / 16\n"
            "  v:Set(0.15 + 0.15 * math.cos(angle), 0.35 + 0.12 * math.sin(angle))\n"
            "  if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end\n"
            "end\n"
            "mesh:WeldPoints(mesh:CountPoints() - 1, mesh:CountPoints() - 16, 0)\n"
            "mesh:SelectNone()\n"
            "for i = mesh:CountPoints() - 16, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end\n"
            "moho:CreateShape(true, false, 0)\n"
            "IPCFileSaveAs(moho, '%s')\n"
            "print('✓ 兔子已保存')\n",
            output_path);
    } else if (strcmp(shape, "puppy") == 0) {
        snprintf(lua_cmd, sizeof(lua_cmd),
            "moho:FileNew()\n"
            "local layer = moho:CreateNewLayer(MOHO.LT_VECTOR)\n"
            "layer:SetName('PuppyLayer')\n"
            "moho:SetSelLayer(layer)\n"
            "local mesh = moho:DrawingMesh()\n"
            "local v = LM.Vector2:new_local()\n"
            "-- 身体（金色）\n"
            "for i = 0, 15 do\n"
            "  local angle = i * math.pi * 2 / 16\n"
            "  v:Set(0.35 * math.cos(angle), 0.25 * math.sin(angle))\n"
            "  if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end\n"
            "end\n"
            "mesh:WeldPoints(15, 0, 0)\n"
            "mesh:SelectNone()\n"
            "for i = 0, 15 do mesh:Point(i).fSelected = true end\n"
            "local shapeID = moho:CreateShape(true, false, 0)\n"
            "if shapeID >= 0 then\n"
            "  local s = mesh:Shape(shapeID)\n"
            "  s.fHasFill = true\n"
            "  s.fMyStyle.fDefineFillCol = true\n"
            "  local col = LM.ColorVector:new_local()\n"
            "  col:Set(0.8, 0.6, 0.2, 1.0)\n"
            "  s.fMyStyle.fFillCol:SetValue(0, col:AsColorStruct())\n"
            "end\n"
            "-- 头部\n"
            "for i = 0, 15 do\n"
            "  local angle = i * math.pi * 2 / 16\n"
            "  v:Set(0.2 + 0.15 * math.cos(angle), 0.4 + 0.12 * math.sin(angle))\n"
            "  if i == 0 then mesh:AddLonePoint(v, 0) else mesh:AppendPoint(v, 0) end\n"
            "end\n"
            "mesh:WeldPoints(mesh:CountPoints() - 1, mesh:CountPoints() - 16, 0)\n"
            "mesh:SelectNone()\n"
            "for i = mesh:CountPoints() - 16, mesh:CountPoints() - 1 do mesh:Point(i).fSelected = true end\n"
            "moho:CreateShape(true, false, 0)\n"
            "IPCFileSaveAs(moho, '%s')\n"
            "print('✓ 小狗已保存')\n",
            output_path);
    }
    
    printf("▶ IPC 绘制中...\n");
    int ret = ipc_send_multiline(lua_cmd);
    
    if (ret == 0) {
        printf("✓ 已保存: %s\n", output_path);
    } else {
        fprintf(stderr, "✗ 绘制失败\n");
    }
    
    return ret;
}