# Moho-Mate 脚本包管理设计 v2 — PnP 架构

> 不模仿 node_modules 目录结构，用 importmap 映射表 + Lua package.path 实现依赖解析

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────┐
│  Store（全局存储）                                         │
│  ~/Library/Application Support/com.maohou.moho-mate/     │
│  唯一数据源，所有用户内容目录共享                              │
└──────────────────────────┬──────────────────────────────┘
                           │ loadfile 直接引用
                           ▼
┌─────────────────────────────────────────────────────────┐
│  用户内容文件夹                                            │
│  ~/moho_user_content/Moho Pro/                           │
│  ├── moho-mate.importmap.json   ← 依赖映射表              │
│  └── Scripts/                   ← Moho 脚本（引用文件）    │
└─────────────────────────────────────────────────────────┘
```

**只有两层**，没有中间层。依赖关系由 importmap 解决，不需要 `.pnpm` 等价物。

---

## 2. 目录结构详解

### 2.1 Store — 全局存储

**位置**：
| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/com.maohou.moho-mate/packages/` |
| Windows | `%LOCALAPPDATA%/com.maohou.moho-mate/packages/` |

**结构**：`组织名/包名/版本号`，唯一数据源。

```
~/Library/Application Support/com.maohou.moho-mate/
├── packages/
│   ├── @maohou/
│   │   ├── json/
│   │   │   ├── 1.0.0/
│   │   │   │   ├── package.json
│   │   │   │   └── Scripts/
│   │   │   │       └── ...
│   │   │   └── 1.1.0/
│   │   │       └── ...
│   │   └── utils/
│   │       └── 2.0.0/
│   │           └── ...
│   └── moho-sdk/
│       └── 1.0.0/
│           ├── package.json
│           └── Scripts/
│               └── ...
└── config.json
```

### 2.2 用户内容文件夹

```
~/moho_user_content/Moho Pro/
├── moho-mate.importmap.json        ← 依赖映射表（同时充当 lock）
└── Scripts/
    ├── Tool/
    │   └── my_tool.lua             ← 引用文件（loadfile → store）
    ├── Menu/
    │   └── Export/
    │       └── helper.lua          ← 引用文件
    ├── Modules/
    │   └── @maohou_json.lua        ← 引用文件
    └── _tool_list.txt
```

**没有软链接**，引用文件直接 `loadfile` 到 store 绝对路径。

---

## 3. moho-mate.importmap.json（核心）

### 3.1 严格遵循 HTML Import Maps 规范

参考 [Import Maps](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script/type/importmap) 规范，只使用两个标准字段：

- **`imports`**：全局映射，裸名/前缀 → store 绝对路径
- **`scopes`**：作用域映射，限定哪个包能访问哪些依赖（解决幽灵依赖）

### 3.2 格式设计

```json
{
  "imports": {
    "@maohou/json": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/json/1.0.0/Scripts/Modules/json.lua",
    "@maohou/json/": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/json/1.0.0/",
    "@maohou/utils/": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/utils/2.0.0/",
    "moho-sdk": "~/Library/Application Support/com.maohou.moho-mate/packages/moho-sdk/1.0.0/Scripts/sdk.lua",
    "moho-sdk/": "~/Library/Application Support/com.maohou.moho-mate/packages/moho-sdk/1.0.0/"
  },
  "scopes": {
    "~/Library/Application Support/com.maohou.moho-mate/packages/moho-sdk/1.0.0/": {
      "@maohou/json": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/json/1.0.0/Scripts/Modules/json.lua",
      "@maohou/json/": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/json/1.0.0/",
      "@maohou/utils/": "~/Library/Application Support/com.maohou.moho-mate/packages/@maohou/utils/2.0.0/"
    }
  }
}
```

### 3.3 字段说明

**`imports`**：全局入口映射

| 键格式 | 条件 | 说明 |
|--------|------|------|
| `裸名` | package.json 有 `main` | 包主入口，值由 main 字段指定 |
| `裸名/` | 始终存在 | 包目录前缀，支持子路径引用 |

> 示例：`@maohou/utils` 没有 `main` 字段，所以 imports 中只有 `"@maohou/utils/"` 前缀映射，没有裸名映射。

**`scopes`**：依赖隔离

| 键 | 说明 |
|----|------|
| scope 键（包路径） | 哪个包在使用 |
| scope 值 | 该包能访问的依赖映射 |

规则：当某个包内部 require 时，先查 scopes[该包路径]，找不到再查 imports。
这确保了 moho-sdk 能看到 `@maohou/json`，但不会泄露给未声明依赖的包。

---

## 4. Lua 依赖解析机制

### 4.1 核心原理：package.preload

**已验证**：Moho Lua 5.4 完全支持 `package.preload`。

Lua `require()` 的查找顺序：

```
require("xxx")
  1. package.loaded   → 已缓存？直接返回
  2. package.preload  → 有注册函数？调用它
  3. package.searchers → 按路径搜索文件
```

**package.preload 是一张 `{模块名 → 加载函数}` 的表**，require 会优先查它。
注册到 preload 的模块，require 直接命中，无需遍历文件系统。

### 4.2 引用文件实现

引用文件由 moho-mate 在安装时自动生成，**零运行时 JSON 依赖**：
1. 安装时将依赖路径硬编码为 `package.preload` 注册语句
2. `loadfile` 加载 store 中的实际脚本

```lua
-- Scripts/Tool/my_tool.lua（引用文件）
-- my_tool.lua (引用文件)
-- 包: moho-sdk@1.0.0
-- 由 moho-mate 自动生成，请勿修改

-- 注册依赖到 package.preload
package.preload["moho-sdk"] = function() return dofile("~/Library/.../moho-sdk/1.0.0/Scripts/sdk.lua") end
package.preload["@maohou/json"] = function() return dofile("~/Library/.../@maohou/json/1.0.0/Scripts/Modules/json.lua") end
package.preload["@maohou/utils"] = function() return dofile("~/Library/.../@maohou/utils/2.0.0/Scripts/Modules/utils.lua") end

-- 加载实际脚本
return loadfile("~/Library/.../moho-sdk/1.0.0/Scripts/Tool/my_tool.lua")()
```

**为什么不在运行时读 importmap.json**：
- Moho 脚本没有内置 JSON 解析器
- 安装时已知所有依赖路径，硬编码更可靠
- 引用文件是生成的，不需要动态性
- importmap.json 是给人看的映射表（调试、审计），不是运行时依赖

### 4.3 为什么用 preload 而不是 package.path

| 方案 | 原理 | 包多时性能 | 侵入性 |
|------|------|-----------|--------|
| `package.path` | 追加路径，require 逐个 open 尝试 | O(n) 慢 | 改全局路径 |
| `package.preload` | 安装时硬编码注册，require 直接命中 | O(1) 快 | 零侵入 ✅ 已实现 |
| 替换 require | 包装原始 require | O(1) 快 | 改全局函数 |

`package.preload` 最优雅：
- **不改 require**：包代码里正常 `require("@maohou/json")`，无感知
- **不改 package.path**：Moho 原生路径不受影响
- **幂等**：`require("LM_Debug")` 等原生模块照常走 searchers
- **性能好**：精确匹配，不走文件系统遍历

### 4.4 importmap 解析逻辑（备选）

如需动态解析（懒加载场景），可插入 `package.searchers`：

```lua
table.insert(package.searchers, 1, function(name)
    local resolved = resolve_from_importmap(name, current_scope)
    if resolved then
        return function() return dofile(resolved) end
    end
    return nil  -- 交给下一个 searcher
end)
```

> **已采用 preload 方案**（安装时一次性注册）。
> 后续如需懒加载（按需注册），可切换到 searchers。

---

## 5. 安装流程

```
moho-mate pkg install ./package.zip

1. 解压到临时目录，读取 package.json

2. 安装到 store
   store/<name>/<version>/  ← 唯一数据

3. 解析并安装依赖（递归）
   检查 store 中是否已有，缺失则从 registry 下载

4. 更新 importmap
   写入 moho-mate.importmap.json：
   - imports: 添加包的裸名（有 main 时）和前缀映射
   - imports: 添加所有依赖的映射
   - scopes: 为每个包添加依赖隔离映射

5. 生成引用文件
   Scripts/ 下的 .lua 文件 → 引用文件（package.preload + loadfile）
   非 .lua 文件 → 直接复制

6. 更新 _tool_list.txt（如有 moho.tools）

7. 完成
```

---

## 6. 与 v1 / pnpm / Yarn PnP 对比

| | v1 | pnpm | Yarn PnP | **v2（本方案）** |
|--|----|------|----------|----------------|
| 全局存储 | ✅ store | ✅ store（hash） | ✅ cache（zip） | ✅ store（路径） |
| 中间层 | 无 | .pnpm | 无 | **无** |
| 依赖解析 | 引用文件硬编码路径 | 目录结构 + 软链接 | .pnp.cjs 映射表 | **importmap.json** |
| 幽灵依赖 | ⚠️ 可能 | ❌ 无 | ❌ 无 | ❌ 无 |
| 版本共存 | ✅ | ✅ | ✅ | ✅ |
| 跨项目共享 | ❌ | ✅ | ✅ | ✅ |
| 文件链接 | 无 | symlink + hardlink | 无 | **无（纯 loadfile）** |

---

## 7. 优势

1. **简单**：只有两层（store + 用户内容），没有软链接/junction 跨平台问题
2. **透明**：importmap.json 人可读，出问题可以直接看
3. **可控**：每个引用文件明确知道自己加载了哪些依赖
4. **兼容**：不依赖文件系统特性，Windows/macOS/Linux 行为一致

---

_文档版本: 2.1.0 | 更新日期: 2026-06-09 | 已通过端到端测试_
