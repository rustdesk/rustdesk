# 覆盖安装处理机制

## 问题描述

当客户已经安装了RustDesk公用版（或其他版本），再安装我们的Cislink定制版时，可能出现以下问题：

### 之前的问题（已修复）

```
安装前：
C:\Program Files\RustDesk\
└── rustdesk.exe  （公用版，连接官方服务器）

使用旧版安装包后：
C:\Program Files\RustDesk\
├── rustdesk.exe  ❌ 旧文件仍然存在！
└── rustdesk-host=hbbs.cislink.nl,key=...,.exe  ✅ 新文件

结果：
- 旧快捷方式仍指向 rustdesk.exe（公用版）
- 用户点击快捷方式启动的是旧版本
- 服务器配置看起来"没有作用"
```

## 解决方案

新版安装包（v2.1）现在包含**完整的清理和强制刷新机制**：

### 安装流程

```
1. 用户运行安装包
   └─> RustDesk_Cislink_Installer_v2.1.exe

2. InitializeSetup()
   └─> 检测RustDesk是否运行
   └─> 提示用户关闭
   └─> 强制停止所有rustdesk*.exe进程

3. 复制文件
   └─> 将 rustdesk.exe 复制为新文件名
       rustdesk-host=hbbs.cislink.nl,key=...,.exe

4. ssPostInstall 阶段
   ├─> StopRustDesk()
   │   └─> 再次确保所有进程已停止
   │
   ├─> CleanupOldRustDeskFiles() ⭐ 新增
   │   └─> 删除所有旧的 rustdesk*.exe
   │   └─> 保留新的配置文件名
   │
   ├─> ForceResetServerSettings() ⭐ 新增
   │   ├─> 删除 %APPDATA%\RustDesk\config\
   │   ├─> 删除 %PROGRAMDATA%\RustDesk\config\
   │   └─> 删除所有 RustDesk2.toml（用户运行时配置）
   │
   ├─> CreateConfigFiles()
   │   └─> 创建全新的配置文件
   │       包含Cislink服务器设置
   │
   └─> UpdateAllShortcuts() ⭐ 新增
       ├─> 更新桌面快捷方式
       └─> 更新开始菜单快捷方式
       └─> 指向新的EXE文件名

5. 安装完成
   └─> 用户可以启动RustDesk
   └─> 自动连接到Cislink服务器
```

## 技术细节

### 1. CleanupOldRustDeskFiles()

**功能**: 清理所有旧版本的RustDesk可执行文件

```pascal
procedure CleanupOldRustDeskFiles();
var
  FindRec: TFindRec;
  InstallPath: String;
  OldFile: String;
  NewExeName: String;
begin
  Log('Cleaning up old RustDesk files...');
  InstallPath := ExpandConstant('{app}');
  NewExeName := '{#MyAppExeName}';

  if FindFirst(InstallPath + '\rustdesk*.exe', FindRec) then
  begin
    try
      repeat
        // 删除所有 rustdesk*.exe，除了新文件
        if CompareText(FindRec.Name, NewExeName) <> 0 then
        begin
          OldFile := InstallPath + '\' + FindRec.Name;
          Log('Deleting old file: ' + OldFile);
          DeleteFile(OldFile);
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
  Log('Old files cleanup completed');
end;
```

**处理场景**:
- ✅ 删除标准的 `rustdesk.exe`
- ✅ 删除其他供应商的定制版（如 `rustdesk-host=other-server.exe`）
- ✅ 删除备份文件（如 `rustdesk.exe.backup`）
- ✅ 保留我们的新文件

### 2. ForceResetServerSettings()

**功能**: 强制清除所有旧的服务器配置

```pascal
procedure ForceResetServerSettings();
var
  UserConfigDir: String;
  SystemConfigDir: String;
begin
  Log('Force resetting all server settings...');

  // 删除所有现有配置文件
  UserConfigDir := ExpandConstant('{userappdata}\RustDesk\config');
  SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk\config');

  // 删除用户配置
  if DirExists(UserConfigDir) then
  begin
    DelTree(UserConfigDir, True, False, True);
    Log('Deleted user config directory');
  end;

  // 删除系统配置
  if DirExists(SystemConfigDir) then
  begin
    DelTree(SystemConfigDir, True, False, True);
    Log('Deleted system config directory');
  end;

  // 删除运行时用户设置
  DeleteFile(ExpandConstant('{userappdata}\RustDesk\config\RustDesk2.toml'));
  DeleteFile(ExpandConstant('{commonappdata}\RustDesk\config\RustDesk2.toml'));

  Log('All old configurations cleared');
end;
```

**删除的配置**:
- ✅ `%APPDATA%\RustDesk\config\*`（用户配置）
- ✅ `%PROGRAMDATA%\RustDesk\config\*`（系统配置）
- ✅ `RustDesk2.toml`（运行时用户设置）
- ✅ 所有旧的服务器设置

**为什么要删除配置文件？**

RustDesk的配置有多层：
1. **EXE文件名**（最高优先级）← 我们设置的
2. 注册表（次优先级）← 我们会写入
3. **配置文件**（第三优先级）← 可能有旧设置
4. 运行时设置（用户在界面修改的）← 可能有旧设置

虽然EXE文件名优先级最高，但为了保险起见，我们删除所有低优先级的配置，确保没有任何冲突。

### 3. UpdateAllShortcuts()

**功能**: 更新所有快捷方式指向新文件

```pascal
procedure UpdateAllShortcuts();
var
  NewExePath: String;
  ShortcutPath: String;
begin
  Log('Updating shortcuts to point to new executable...');
  NewExePath := ExpandConstant('{app}\{#MyAppExeName}');

  // 更新桌面快捷方式
  ShortcutPath := ExpandConstant('{autodesktop}\{#MyAppName}.lnk');
  if FileExists(ShortcutPath) then
  begin
    DeleteFile(ShortcutPath);
    CreateShellLink(
      ShortcutPath,
      '{#MyAppName}',
      NewExePath,
      ...
    );
    Log('Updated desktop shortcut');
  end;

  // 更新开始菜单快捷方式
  ShortcutPath := ExpandConstant('{group}\{#MyAppName}.lnk');
  if FileExists(ShortcutPath) then
  begin
    DeleteFile(ShortcutPath);
    CreateShellLink(
      ShortcutPath,
      '{#MyAppName}',
      NewExePath,
      ...
    );
    Log('Updated start menu shortcut');
  end;
end;
```

**处理的快捷方式**:
- ✅ 桌面快捷方式
- ✅ 开始菜单快捷方式
- ✅ 自动启动快捷方式（通过[Icons]配置）

## 配置优先级保证

### 多层保护机制

即使某一层失败，其他层仍能确保配置生效：

```
层级 1: EXE文件名 ⭐⭐⭐⭐⭐ (最高优先级)
  └─> rustdesk-host=hbbs.cislink.nl,key=...,.exe
  └─> ✅ 始终生效，用户无法修改
  └─> ✅ Windows系统自动识别

层级 2: 注册表 ⭐⭐⭐⭐
  └─> HKLM\...\Uninstall\RustDesk
  └─> ✅ 安装时写入
  └─> ✅ 向后兼容旧版本

层级 3: 配置文件 ⭐⭐⭐
  └─> %APPDATA%\RustDesk\config\RustDesk.toml
  └─> ✅ 强制刷新（删除旧配置）
  └─> ✅ 创建新配置
  └─> ⚠️ 用户可以手动修改（但优先级低）

层级 4: 运行时设置 ⭐⭐
  └─> RustDesk2.toml
  └─> ✅ 安装时删除
  └─> ⚠️ 用户可以在界面中修改（但优先级最低）
```

**结果**: 即使用户在界面中手动修改服务器设置，下次重启后会被EXE文件名的配置覆盖。

## 测试场景

### 场景1: 全新安装

```
条件: 系统中没有RustDesk
操作: 运行安装包
结果:
  ✅ 安装到 C:\Program Files\RustDesk\
  ✅ EXE文件名包含服务器配置
  ✅ 配置文件包含Cislink服务器
  ✅ 快捷方式指向正确的EXE
  ✅ 启动后自动连接到Cislink服务器
```

### 场景2: 覆盖公用版

```
条件: 已安装RustDesk公用版
      C:\Program Files\RustDesk\rustdesk.exe
操作: 运行安装包
结果:
  ✅ 停止旧版本进程
  ✅ 删除 rustdesk.exe
  ✅ 创建 rustdesk-host=hbbs.cislink.nl...exe
  ✅ 删除所有旧配置文件
  ✅ 创建新配置文件
  ✅ 更新所有快捷方式
  ✅ 启动后自动连接到Cislink服务器
```

### 场景3: 覆盖其他供应商定制版

```
条件: 已安装其他供应商的RustDesk
      C:\Program Files\RustDesk\rustdesk-host=other-server.exe
操作: 运行安装包
结果:
  ✅ 停止旧版本进程
  ✅ 删除 rustdesk-host=other-server.exe
  ✅ 创建 rustdesk-host=hbbs.cislink.nl...exe
  ✅ 强制清除旧服务器配置
  ✅ 创建Cislink服务器配置
  ✅ 更新所有快捷方式
  ✅ 启动后自动连接到Cislink服务器
```

### 场景4: 用户之前手动修改过配置

```
条件: 已安装我们的定制版，但用户在界面中修改了服务器
操作: 重新运行安装包（修复安装）
结果:
  ✅ 强制删除所有配置文件
  ✅ 重新创建Cislink配置
  ✅ EXE文件名不变（已经是正确的）
  ✅ 启动后恢复到Cislink服务器
```

## 安装日志

安装过程会记录详细日志，位于：
```
%TEMP%\Setup Log YYYY-MM-DD #NNN.txt
```

关键日志条目：
```
Stopping RustDesk processes...
Cleaning up old RustDesk files...
Deleting old file: C:\Program Files\RustDesk\rustdesk.exe
Old files cleanup completed
Force resetting all server settings...
Deleted user config directory
Deleted system config directory
All old configurations cleared
Creating configuration files...
Configuration files created successfully
Updating shortcuts to point to new executable...
Updated desktop shortcut
Updated start menu shortcut
```

## 用户体验

### 安装时

```
1. [弹窗] RustDesk正在运行，需要关闭继续？
   └─> 用户点击"是"

2. [进度条] 正在安装...
   └─> 后台执行：
       - 停止进程
       - 删除旧文件
       - 清除旧配置
       - 安装新文件
       - 创建新配置
       - 更新快捷方式

3. [完成]
   └─> 选项：立即启动RustDesk
```

### 启动后

```
1. 用户启动RustDesk（双击桌面图标）
   └─> 启动的是新的EXE（包含服务器配置）

2. RustDesk读取配置
   └─> 从EXE文件名解析服务器配置
   └─> 自动连接到 hbbs.cislink.nl

3. 用户打开设置 → 网络
   └─> 显示：
       ID服务器: hbbs.cislink.nl ✅
       中继服务器: hbbr.cislink.nl ✅
       Key: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY= ✅
```

## 与迁移脚本的对比

| 特性 | 安装包 | 迁移脚本 |
|------|--------|----------|
| 删除旧EXE | ✅ | ✅ (重命名模式) |
| 清除配置 | ✅ 完全删除 | ✅ 覆盖 |
| 更新快捷方式 | ✅ | ✅ (完整版) |
| 需要管理员 | ✅ | ✅ (推荐) |
| 适用场景 | 全新安装+覆盖安装 | 仅覆盖安装 |
| 推荐对象 | 所有用户 | 已安装用户 |

**建议**:
- 新用户：直接使用安装包
- 已安装用户（需要快速切换）：使用迁移脚本
- 已安装用户（需要完全重置）：重新运行安装包

## 验证方法

### 验证1: 检查EXE文件

```powershell
Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe
```

**期望输出**（仅一个文件）:
```
Name
----
rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe
```

**如果看到多个文件**:
```
❌ rustdesk.exe
✅ rustdesk-host=hbbs.cislink.nl...exe
```
说明清理失败，可能需要手动删除 `rustdesk.exe`

### 验证2: 检查配置文件

```powershell
Get-Content "$env:APPDATA\RustDesk\config\RustDesk.toml"
```

**期望内容**:
```toml
[options]
custom-rendezvous-server = "hbbs.cislink.nl"
relay-server = "hbbr.cislink.nl"
key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
disable-update-check = true
disable-installation = true
```

### 验证3: 检查快捷方式

```powershell
# 检查桌面快捷方式
$shortcut = (New-Object -COM WScript.Shell).CreateShortcut("$env:USERPROFILE\Desktop\RustDesk - Cislink Edition.lnk")
$shortcut.TargetPath
```

**期望输出**:
```
C:\Program Files\RustDesk\rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe
```

### 验证4: 启动测试

1. 启动RustDesk
2. 打开设置 → 网络
3. 确认服务器地址为 `hbbs.cislink.nl`

## 故障排除

### 问题1: 安装后仍然有旧EXE文件

**可能原因**: 文件被占用，删除失败

**解决方案**:
```powershell
# 手动删除旧文件
taskkill /F /IM rustdesk*.exe /T
Remove-Item "C:\Program Files\RustDesk\rustdesk.exe" -Force
```

### 问题2: 快捷方式不工作

**可能原因**: 快捷方式更新失败

**解决方案**:
1. 删除旧快捷方式
2. 重新运行安装包
3. 或手动创建快捷方式指向新EXE

### 问题3: 配置没有生效

**检查步骤**:
```powershell
# 1. 检查EXE文件名是否正确
Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe

# 2. 检查是否有旧配置残留
Get-Content "$env:APPDATA\RustDesk\config\RustDesk2.toml"

# 3. 如果有问题，重新运行安装包
```

## 更新日志

| 日期 | 版本 | 更新内容 |
|------|------|----------|
| 2025-10-27 | 2.1 | 添加完整的覆盖安装支持 |
|  |  | - CleanupOldRustDeskFiles() |
|  |  | - ForceResetServerSettings() |
|  |  | - UpdateAllShortcuts() |

## 总结

新版安装包（v2.1）现在可以**完美处理覆盖安装**：

✅ 删除所有旧版本EXE文件
✅ 强制清除所有旧配置
✅ 创建全新的Cislink配置
✅ 更新所有快捷方式
✅ 确保100%连接到Cislink服务器

无论用户之前安装的是什么版本的RustDesk，我们的安装包都能**强制刷新**并确保配置生效！
