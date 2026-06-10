# RustDesk安装包服务器配置修复文档

## 问题描述

之前创建的RustDesk安装包虽然在安装后会创建配置文件（`RustDesk.toml`），但安装后的RustDesk客户端仍然连接到**公用版本的服务器**，而不是我们自定义的Cislink服务器。

## 根本原因

通过分析RustDesk源代码（`src/custom_server.rs` 和 `src/platform/windows.rs`），发现：

### Windows平台的特殊配置机制

在Windows系统上，RustDesk使用**特殊的优先级顺序**来读取服务器配置：

```
1. EXE文件名中嵌入的配置（最高优先级）
   └─> 通过 get_license_from_exe_name() 函数解析
2. 用户自定义配置文件
3. PROD_RENDEZVOUS_SERVER 变量
4. 编译时环境变量 RENDEZVOUS_SERVER
```

### 关键代码证据

**文件名解析逻辑**（`src/custom_server.rs:39-88`）:
```rust
pub fn get_custom_server_from_string(s: &str) -> ResultType<CustomServer> {
    // 支持格式: rustdesk-host=服务器,key=密钥,relay=中继.exe
    if s.to_lowercase().contains("host=") {
        // 解析host, key, api, relay参数
        ...
    }
}
```

**启动时读取**（`src/platform/windows.rs:1789-1791`）:
```rust
pub fn bootstrap() -> bool {
    if let Ok(lic) = get_license_from_exe_name() {
        *config::EXE_RENDEZVOUS_SERVER.write().unwrap() = lic.host.clone();
    }
    ...
}
```

**服务器获取优先级**（`src/common.rs:970-984`）:
```rust
pub fn get_custom_rendezvous_server(custom: String) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.host.is_empty() {
            return lic.host.clone();  // 最高优先级！
        }
    }
    if !custom.is_empty() {
        return custom;
    }
    if !config::PROD_RENDEZVOUS_SERVER.read().unwrap().is_empty() {
        return config::PROD_RENDEZVOUS_SERVER.read().unwrap().clone();
    }
    "".to_owned()
}
```

## 修复方案

### 修改内容

修改Inno Setup安装脚本（`RustDesk-Installer.iss`），在安装时将`rustdesk.exe`重命名为包含服务器配置的文件名。

### 支持的文件名格式

根据源代码测试用例（`src/custom_server.rs:114-218`），RustDesk支持以下文件名格式：

#### 方法1：明文配置（推荐）
```
rustdesk-host=服务器地址,key=公钥,relay=中继服务器.exe
```

**我们使用的配置**:
```
rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe
```

注意：
- 参数不区分大小写（`host=`、`Host=`、`HOST=`都可以）
- 最后可以加逗号（`,.exe`）用于避免Windows重命名文件时的问题
- 参数顺序不重要，但`host=`通常放在第一位

#### 方法2：加密配置
```
rustdesk-licensed-<加密字符串>.exe
rustdesk--<加密字符串>.exe
```

这种方式需要使用RustDesk的私钥签名配置，较为复杂。

### 具体修改

**1. 修改可执行文件名定义**：
```pascal
#define MyAppExeName "rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe"
```

**2. 修改文件复制规则**：
```pascal
[Files]
Source: "rustdesk.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
```

**3. 更新进程停止逻辑**：
```pascal
// 使用通配符停止所有rustdesk进程
Exec('taskkill', '/F /IM rustdesk*.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
```

## 验证步骤

### 安装前验证
```powershell
# 运行测试脚本
powershell -ExecutionPolicy Bypass -File test-installer.ps1
```

### 安装后验证

1. **检查可执行文件名**:
   ```powershell
   Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe | Select-Object Name
   ```

   期望输出：
   ```
   Name
   ----
   rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe
   ```

2. **启动RustDesk并检查连接**:
   - 启动RustDesk客户端
   - 打开设置 -> 网络
   - 确认显示的服务器为：
     - ID服务器: `hbbs.cislink.nl`
     - 中继服务器: `hbbr.cislink.nl`
     - 公钥: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`

3. **检查日志**（可选）:
   ```powershell
   Get-Content "$env:APPDATA\RustDesk\logs\*.log" | Select-String "hbbs.cislink.nl"
   ```

## 技术细节

### 为什么配置文件不起作用？

虽然安装脚本在`ssPostInstall`阶段会创建配置文件：
```
%APPDATA%\RustDesk\config\RustDesk.toml
%APPDATA%\RustDesk\config\RustDesk2.toml
%PROGRAMDATA%\RustDesk\config\RustDesk.toml
%PROGRAMDATA%\RustDesk\config\RustDesk2.toml
```

但是在Windows平台上，**EXE文件名的配置优先级更高**。因此：
- 如果EXE文件名中没有配置 → 使用默认公共服务器
- 如果EXE文件名中有配置 → 使用文件名中的配置（忽略配置文件）

### Windows文件重命名保护

RustDesk的文件名解析考虑了Windows的自动重命名机制：
- 当文件重复时，Windows会添加` (1)`、` (2)`等后缀
- 通过在配置后添加逗号（`,.exe`），可以确保即使被重命名为`rustdesk-host=...,.exe (1).exe`，配置仍然可以正确解析

### 配置解析流程

```
rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe
                                                                                          ^
                                                                                          |
1. 去掉.exe后缀 ──────────────────────────────────────────────────────────────────────────┘
2. 检测到包含 "host="
3. 按逗号分割: ["rustdesk-host=hbbs.cislink.nl", "key=...", "relay=hbbr.cislink.nl", ""]
4. 解析每个参数:
   - host = "hbbs.cislink.nl"
   - key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
   - relay = "hbbr.cislink.nl"
5. 创建 CustomServer 结构体
6. 在启动时设置 EXE_RENDEZVOUS_SERVER
```

## 构建新的安装包

```powershell
# 确保在RustDesk目录
cd D:\Rustdesk

# 编译安装包
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"

# 输出位置
# D:\Rustdesk\Output\RustDesk_Cislink_Installer_v2.1.exe
```

## 服务器配置

当前使用的Cislink服务器配置：

| 配置项 | 值 |
|--------|-----|
| ID服务器 | hbbs.cislink.nl |
| 中继服务器 | hbbr.cislink.nl |
| 公钥 | VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY= |

## 修改历史

| 日期 | 版本 | 修改内容 |
|------|------|----------|
| 2025-10-27 | 2.1 | 修复服务器配置问题，使用文件名嵌入配置 |
| 之前 | 2.0 | 初始版本，使用配置文件（未生效） |

## 参考资料

- RustDesk源代码: `src/custom_server.rs`
- Windows平台代码: `src/platform/windows.rs`
- 配置获取逻辑: `src/common.rs`
- 官方文档: https://rustdesk.com/docs/en/self-host/install/

## 故障排除

### 问题1: 安装后仍连接到公共服务器

**可能原因**:
1. EXE文件名不正确
2. 旧版本的RustDesk仍在运行

**解决方案**:
```powershell
# 1. 停止所有RustDesk进程
taskkill /F /IM rustdesk*.exe /T

# 2. 检查EXE文件名
Get-ChildItem "C:\Program Files\RustDesk\" -Filter rustdesk*.exe

# 3. 重新安装
```

### 问题2: 无法启动RustDesk

**可能原因**: 文件名过长

**解决方案**: Windows有260字符路径限制，当前配置文件名长度安全。

### 问题3: 配置被用户修改

如果用户在界面中修改了服务器设置，这些修改会被保存到配置文件中，但**不会覆盖**EXE文件名中的配置。下次重启时，仍会使用EXE文件名中的配置。

要完全锁定配置，需要：
1. 使用组策略或注册表限制
2. 或者编译时内置配置（需要重新编译RustDesk）
