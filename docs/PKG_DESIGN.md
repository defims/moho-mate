# Moho-Mate 脚本包管理设计文档

> 类似 npm/pnpm 的 Moho 脚本包管理系统

---

## 1. 概述

### 1.1 目标

- 提供标准化的脚本包分发机制
- 支持依赖管理
- 干净的安装/卸载
- 多版本共存

### 1.2 设计原则

- **不污染 Moho 系统脚本**：所有用户脚本都在用户内容文件夹
- **引用文件机制**：实际代码存储在中央目录，用户文件夹放引用
- **Lock 文件保证一致性**：类似 package-lock.json

---

## 2. 目录结构

### 2.1 包存储目录

```
~/Library/Application Support/com.maohou.moho-mate/
├── config.json                    # 配置文件
├── moho-mate-lock.json           # Lock 文件
└── packages/
    ├── @maohou/
    │   └── json/
    │       ├── 1.0.0/
    │       │   ├── package.json
    │       │   └── Scripts/
    │       │       ├── Modules/
    │       │       │   └── init.lua
    │       │       └── Tool/
    │       │           ├── my_tool.lua
    │       │           └── my_tool.png
    │       └── 1.1.0/
    │           └── ...
    └── utils/
        └── 2.0.0/
            └── ...
```

### 2.2 用户内容文件夹（引用文件）

```
/Users/{user}/moho_user_content/Moho Pro/Scripts/
├── Modules/
│   └── @maohou_json.lua          # 引用文件（loadfile）
├── Tool/
│   ├── my_tool.lua               # 引用文件（loadfile）
│   ├── my_tool.png               # 符号链接或复制
│   └── _tool_list.txt            # Tool 列表
└── Menu/
    └── Export/
        └── helper.lua             # 引用文件（loadfile）
```

### 2.3 平台差异

| 平台 | 包存储目录 |
|------|-----------|
| macOS | `~/Library/Application Support/com.maohou.moho-mate/packages/` |
| Windows | `%LOCALAPPDATA%/com.maohou.moho-mate/packages/` |
| Linux | `~/.local/share/com.maohou.moho-mate/packages/` |

| 平台 | 用户内容文件夹 |
|------|---------------|
| macOS | `~/moho_user_content/Moho Pro/Scripts/` |
| Windows | `Documents/Moho Pro/Scripts/` |
| Linux | `~/.local/share/Moho Pro/Scripts/` |

---

## 3. package.json 规范

### 3.1 字段说明

| 字段 | 必填 | 条件 | 说明 |
|------|:----:|------|------|
| `name` | ✅ | - | 包名，支持 `@org/name` 格式 |
| `version` | ✅ | - | 语义化版本号 |
| `description` | ❌ | - | 包描述 |
| `author` | ❌ | - | 作者 |
| `license` | ❌ | - | 许可证 |
| `main` | ❌ | 被依赖时必填 | 主入口文件 |
| `files` | ✅ | - | 文件清单 |
| `exports` | ❌ | - | 子路径导出定义 |
| `dependencies` | ❌ | - | 依赖包 |
| `moho` | ❌ | - | Moho 特定配置 |

### 3.2 Moho 配置

```json
{
  "moho": {
    "min_version": "14.0",
    "max_version": "14.9",
    "tools": [
      { "id": "my_tool", "name": "My Tool" }
    ]
  }
}
```

| 字段 | 说明 |
|------|------|
| `min_version` | 最低 Moho 版本 |
| `max_version` | 最高 Moho 版本 |
| `tools` | Tool 脚本清单（id 对应文件名，不含 .lua） |

### 3.3 验证规则

```rust
impl PackageJson {
    pub fn validate(&self) -> Result<()> {
        // 1. name 和 version 必填
        require_field(&self.name, "name")?;
        require_field(&self.version, "version")?;
        
        // 2. files 必填且非空
        require_field(&self.files, "files")?;
        
        // 3. main 指向的文件必须在 files 中
        if let Some(ref main) = self.main {
            ensure_file_in_list(main, &self.files)?;
        }
        
        // 4. exports 指向的文件必须在 files 中
        if let Some(ref exports) = self.exports {
            for path in exports.values() {
                ensure_file_in_list(path, &self.files)?;
            }
        }
        
        // 5. Tool 脚本必须在 files 中
        if let Some(ref moho) = self.moho {
            for tool in &moho.tools {
                ensure_tool_in_files(&tool.id, &self.files)?;
            }
        }
        
        Ok(())
    }
}
```

---

## 4. 引用文件机制

### 4.1 为什么用引用文件？

- **避免代码重复**：多个项目可以引用同一个包
- **支持多版本**：不同版本并存，用户选择使用哪个
- **易于管理**：卸载只需删除引用文件和包目录

### 4.2 引用文件格式

```lua
-- Tool/my_tool.lua（引用文件）
-- 由 moho-mate 自动生成

-- 添加本包和依赖的 Modules 到 package.path
local path = "/path/to/packages/@maohou/json/1.0.0/Scripts/Modules/?.lua"
if not package.path:find(path, 1, true) then
    package.path = path .. ";" .. package.path
end

-- 加载实际脚本
return loadfile("/path/to/packages/@maohou/json/1.0.0/Scripts/Tool/my_tool.lua")()
```

### 4.3 require 支持规则

| 配置 | `require("@包名")` | `require("@包名/子路径")` | `require("模块名")` |
|------|:------------------:|:------------------------:|:------------------:|
| 有 main | ✅ | ❌（无 exports） | ✅（Modules 目录） |
| 有 exports["."] | ✅ | ✅（有定义的子路径） | ✅（Modules 目录） |
| 有 exports（无 "."） | ❌ | ✅（有定义的子路径） | ✅（Modules 目录） |
| 都没有 | ❌ | ❌ | ✅（Modules 目录） |

---

## 5. 安装流程

```
┌─────────────────────────────────────────────────────────────┐
│                      pkg install <package>                    │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  本地文件安装？   │
                    └─────────────────┘
                      │           │
                     是           否
                      │           │
                      ▼           ▼
            ┌─────────────┐  ┌──────────────┐
            │  解压 ZIP    │  │ 下载包 tar.gz │
            └─────────────┘  └──────────────┘
                      │           │
                      └─────┬─────┘
                            ▼
                  ┌──────────────────┐
                  │  解析 package.json │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │   验证 package    │
                  │  - files 存在性   │
                  │  - main/exports   │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  检查是否已安装    │
                  │  （同版本跳过）    │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  递归安装依赖      │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │ 复制文件到包目录   │
                  │ packages/{n}/{v}/ │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  生成引用文件      │
                  │  - Modules/*.lua  │
                  │  - Tool/*.lua     │
                  │  - Menu/**/*.lua  │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │ 更新 _tool_list.txt│
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  更新 lock 文件    │
                  └──────────────────┘
                            │
                            ▼
                        ✓ 完成
```

---

## 6. 卸载流程

```
┌─────────────────────────────────────────────────────────────┐
│                    pkg uninstall <package>                   │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
                  ┌──────────────────┐
                  │  检查包是否已安装  │
                  └──────────────────┘
                            │
                      不存在 ──────→ ✗ 错误
                            │
                            ▼
                  ┌──────────────────┐
                  │  检查依赖包        │
                  │  （是否有其他包   │
                  │   依赖此包）       │
                  └──────────────────┘
                            │
                      有依赖 ──────→ ⚠ 警告 + 确认
                            │
                            ▼
                  ┌──────────────────┐
                  │  删除引用文件      │
                  │  - Modules/*.lua  │
                  │  - Tool/*.lua     │
                  │  - Tool/*.png     │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │ 更新 _tool_list.txt│
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  删除包目录        │
                  │ packages/{n}/{v}/ │
                  └──────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │  更新 lock 文件    │
                  └──────────────────┘
                            │
                            ▼
                        ✓ 完成
```

---

## 7. Lock 文件格式

```json
{
  "version": 1,
  "packages": {
    "@maohou/json": {
      "version": "1.0.0",
      "resolved": "file:./json-1.0.0.zip",
      "integrity": "sha512-...",
      "dependencies": {
        "@maohou/utils": "^1.0.0"
      }
    },
    "@maohou/utils": {
      "version": "1.2.0",
      "resolved": "https://registry.npmjs.org/@maohou/utils/-/utils-1.2.0.tgz",
      "integrity": "sha512-..."
    }
  }
}
```

---

## 8. 配置文件格式

```json
{
  "version": 1,
  "registry": "https://mirrors.cloud.tencent.com/npm",
  "packages_dir": "~/Library/Application Support/com.maohou.moho-mate/packages",
  "user_content_dir": "~/moho_user_content/Moho Pro/Scripts"
}
```

---

## 9. 命令清单

### 9.1 P0 优先级（已实现）

```bash
# 配置管理
moho-mate pkg config list              # 显示配置
moho-mate pkg config set registry <url> # 设置 registry

# 安装/卸载
moho-mate pkg install ./package.zip    # 安装本地包
moho-mate pkg uninstall <package>      # 卸载包

# 查询
moho-mate pkg list                     # 列出已安装的包
moho-mate pkg info <package>           # 显示包信息
```

### 9.2 P1 优先级（待实现）

```bash
# Registry 操作
moho-mate pkg install <package>        # 从 registry 安装
moho-mate pkg info <package>           # 从 registry 获取信息
moho-mate pkg search <keyword>         # 搜索包
moho-mate pkg deps <package>           # 显示依赖树
```

### 9.3 P2 优先级（待实现）

```bash
# 开发命令
moho-mate pkg create                   # 创建包模板
moho-mate pkg pack                     # 打包为 zip
moho-mate pkg publish                  # 发布到 registry

# 维护命令
moho-mate pkg update [<package>]       # 更新包
moho-mate pkg prune                    # 清理无用包
```

---

## 10. 示例输出

### 10.1 安装包

```
$ moho-mate pkg install ./moho-sdk-1.0.0.zip
▶ 安装: ./moho-sdk-1.0.0.zip
  解析 package.json...
  验证文件...
  
▶ 安装依赖:
  @maohou/json@^1.0.0
  @maohou/utils@^2.0.0
  
✓ 已安装: @maohou/json@1.0.0
✓ 已安装: @maohou/utils@2.1.0

▶ 生成引用文件:
  Scripts/Tool/my_tool.lua ✓
  Scripts/Tool/my_tool.png ✓
  Scripts/Modules/init.lua ✓

▶ 更新 _tool_list.txt:
  + my_tool

✓ 已安装: moho-sdk@1.0.0
```

### 10.2 列出包

```
$ moho-mate pkg list

=== 已安装的脚本包 ===

@maohou/json@1.0.0
  描述: JSON 解析库
  工具: json_formatter

@maohou/utils@2.1.0
  描述: 工具函数库
  路径: ~/Library/Application Support/.../@maohou/utils/2.1.0/

moho-sdk@1.0.0
  描述: Moho SDK
  工具: my_tool, helper
  路径: ~/Library/Application Support/.../moho-sdk/1.0.0/

共 3 个包
```

### 10.3 卸载包

```
$ moho-mate pkg uninstall @maohou/json
▶ 卸载: @maohou/json

⚠ 警告: 以下包依赖此包:
  - moho-sdk@1.0.0
  
继续卸载？[y/N] y

▶ 删除引用文件:
  Scripts/Modules/@maohou_json.lua ✓
  Scripts/Tool/json_formatter.lua ✓

▶ 更新 _tool_list.txt:
  - json_formatter

✓ 卸载完成
```

---

## 11. 错误处理

```bash
# 包未找到
✗ 包未找到: @maohou/unknown
  提示: 使用 'moho-mate pkg search' 搜索可用包

# 版本不满足
✗ 版本不满足: @maohou/json@^2.0.0
  需要: >=2.0.0 <3.0.0
  可用: 1.0.0, 1.5.0
  提示: 使用 'moho-mate pkg install @maohou/json@1.5.0' 安装兼容版本

# 文件缺失
✗ 包验证失败: my-package
  文件不存在: Scripts/Tool/my_tool.lua
  提示: 检查 package.json 中的 files 字段

# 依赖冲突
✗ 依赖冲突:
  moho-sdk 需要 @maohou/json@^1.0.0
  另一个包 需要 @maohou/json@^2.0.0
  提示: 使用 'moho-mate pkg deps' 查看依赖树
```

---

## 12. Registry 集成

### 12.1 默认 Registry

```
https://mirrors.cloud.tencent.com/npm
```

### 12.2 包命名约定

```
@maohou/xxx    # 官方包
@{org}/xxx     # 组织包
xxx            # 社区包
```

### 12.3 包查找顺序

1. 本地缓存（`packages/` 目录）
2. Registry 缓存
3. Registry 下载

### 12.4 认证（可选）

```bash
# 设置认证 token
moho-mate pkg config set auth.token "xxx"

# 或使用 .npmrc
# ~/.npmrc
//registry.npmjs.org/:_authToken=xxx
```

---

## 13. 实现状态

| 功能 | 状态 | 优先级 |
|------|:----:|:------:|
| 本地文件安装 | ✅ | P0 |
| 卸载包 | ✅ | P0 |
| 列出已安装包 | ✅ | P0 |
| 显示包信息 | ✅ | P0 |
| 设置 registry | ✅ | P0 |
| 显示依赖树 | ✅ | P0 |
| 从 registry 安装 | ✅ | P1 |
| 搜索包 | ✅ | P1 |
| 更新包 | ✅ | P2 |
| 清理无用包 | ✅ | P2 |
| 创建包模板 | ✅ | P2 |
| 打包发布 | ✅ | P2 |

---

## 附录 A: 包结构示例

```
moho-sdk-1.0.0.zip
└── moho-sdk-1.0.0/
    ├── package.json
    └── Scripts/
        ├── Modules/
        │   ├── init.lua          # 包入口
        │   └── utils.lua         # 工具模块
        ├── Tool/
        │   ├── my_tool.lua       # Tool 脚本
        │   └── my_tool.png       # Tool 图标
        └── Menu/
            └── Export/
                └── helper.lua    # Menu 脚本
```

---

## 附录 B: 引用文件示例

### B.1 Tool 引用文件

```lua
-- ~/moho_user_content/Moho Pro/Scripts/Tool/my_tool.lua
-- 由 moho-mate 自动生成，请勿修改

-- 添加依赖包的 Modules 到 package.path
local pkg_path = "/Users/def/Library/Application Support/com.maohou.moho-mate/packages/moho-sdk/1.0.0/Scripts/Modules/?.lua"
if not package.path:find(pkg_path, 1, true) then
    package.path = pkg_path .. ";" .. package.path
end

-- 加载实际脚本
local script_path = "/Users/def/Library/Application Support/com.maohou.moho-mate/packages/moho-sdk/1.0.0/Scripts/Tool/my_tool.lua"
return loadfile(script_path)()
```

### B.2 Modules 引用文件

```lua
-- ~/moho_user_content/Moho Pro/Scripts/Modules/@maohou_json.lua
-- 由 moho-mate 自动生成，请勿修改

return loadfile("/Users/def/Library/Application Support/com.maohou.moho-mate/packages/@maohou/json/1.0.0/Scripts/Modules/init.lua")()
```

---

_文档版本: 1.0.0 | 更新日期: 2026-06-08_
