# 🚀 快速开始 - 打包 RustDesk 客户端

## 步骤 1: 运行打包脚本

```powershell
.\Build-RustDesk-Installer.ps1
```

脚本会自动：
1. ✅ 下载最新的 RustDesk 客户端
2. ✅ 创建预配置文件（包含服务器地址和密钥）
3. ✅ 使用 Inno Setup 编译安装包
4. ✅ 输出到 `RustDesk_Cislink_Setup.exe`

## 步骤 2: 测试安装包

```powershell
# 在虚拟机或测试电脑上运行
.\Output\RustDesk_Cislink_Installer_v1.0.exe
```

## 步骤 3: 分发给用户

将 `RustDesk_Cislink_Setup.exe` 发送给用户，他们运行后：
- ✅ 自动配置服务器: `hbbs.cislink.nl`
- ✅ 自动配置密钥: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`
- ✅ 无需手动设置，开箱即用！

---

## 📝 当前服务器配置

```
ID 服务器:   hbbs.cislink.nl
中继服务器:  hbbr.cislink.nl
公钥:        VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
```

## 🔄 如果密钥更改

1. 获取新密钥:
   ```powershell
   .\Deploy-RustDesk-Docker.ps1 -GetKey
   ```

2. 更新 `Build-RustDesk-Installer.ps1` 中的 `$ServerKey`

3. 重新打包:
   ```powershell
   .\Build-RustDesk-Installer.ps1 -SkipDownload
   ```

---

详细文档: [CLIENT_PACKAGING_GUIDE.md](CLIENT_PACKAGING_GUIDE.md)
