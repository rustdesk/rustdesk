# GitHub Actions 使用指南 - RustDesk 客户端配置安装程序

## 📋 目录
- [概述](#概述)
- [自动触发构建](#自动触发构建)
- [手动触发构建](#手动触发构建)
- [下载构建产物](#下载构建产物)
- [创建 Release](#创建-release)
- [故障排除](#故障排除)

---

## 🎯 概述

GitHub Actions 工作流 `build-config-installer.yml` 会自动构建 RustDesk 客户端配置安装程序。

**构建内容:**
- `RustDesk_Config_Installer.exe` - Windows 安装程序
- `CHECKSUMS.txt` - SHA256 和 MD5 校验和
- `RELEASE_NOTES.md` - 发布说明

**工作流文件位置:**
```
.github/workflows/build-config-installer.yml
```

---

## 🔄 自动触发构建

工作流会在以下情况下自动运行:

### 1. Push 到主分支
当推送以下文件的更改时:
```bash
git add Deploy-RustDeskConfig.ps1
git add Deploy-RustDesk2Config.ps1
git add Deploy-Config.iss
git add RustDesk.toml
git add RustDesk2.toml
git commit -m "Update RustDesk configuration"
git push origin master
```

### 2. Pull Request
当创建 PR 修改配置文件时,会自动构建以验证。

### 3. 推送 Tag (创建 Release)
```bash
# 创建并推送 tag
git tag v1.4.0
git push origin v1.4.0

# 会自动创建 GitHub Release 并上传安装程序
```

---

## 🎮 手动触发构建

### 方法 1: 通过 GitHub Web UI

1. **访问 GitHub 仓库**
   ```
   https://github.com/CislinkNL/rustdesk/actions
   ```

2. **选择工作流**
   - 点击左侧 "Build RustDesk Client Config Installer"

3. **运行工作流**
   - 点击右上角 "Run workflow" 按钮
   - 分支: `master` (默认)
   - **可选输入**:
     - `public_key`: 新的 RustDesk Server Public Key (留空使用现有配置)
     - `version`: 安装程序版本号 (例如: `1.4.0`)
   - 点击 "Run workflow" 确认

4. **监控构建进度**
   - 构建会出现在工作流运行列表中
   - 点击查看详细日志

### 方法 2: 使用 GitHub CLI

```bash
# 安装 GitHub CLI
# https://cli.github.com/

# 登录
gh auth login

# 运行工作流 (使用现有配置)
gh workflow run build-config-installer.yml

# 运行工作流 (指定新 Public Key 和版本)
gh workflow run build-config-installer.yml \
  -f public_key="YOUR_NEW_PUBLIC_KEY_HERE" \
  -f version="1.4.1"

# 查看运行状态
gh run list --workflow=build-config-installer.yml

# 查看最新运行的详细信息
gh run view
```

### 方法 3: 使用 REST API

```bash
# 设置变量
GITHUB_TOKEN="your_personal_access_token"
REPO_OWNER="CislinkNL"
REPO_NAME="rustdesk"

# 触发工作流 (使用现有配置)
curl -X POST \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/actions/workflows/build-config-installer.yml/dispatches \
  -d '{"ref":"master"}'

# 触发工作流 (指定新 Public Key)
curl -X POST \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/actions/workflows/build-config-installer.yml/dispatches \
  -d '{
    "ref":"master",
    "inputs":{
      "public_key":"YOUR_NEW_PUBLIC_KEY_HERE",
      "version":"1.4.1"
    }
  }'
```

---

## 📥 下载构建产物

### 方法 1: 从 GitHub Web UI 下载

1. **访问 Actions 页面**
   ```
   https://github.com/CislinkNL/rustdesk/actions
   ```

2. **选择工作流运行**
   - 点击最近的 "Build RustDesk Client Config Installer" 运行

3. **下载 Artifacts**
   - 滚动到页面底部的 "Artifacts" 部分
   - 点击 `rustdesk-config-installer-v1.4.0` 下载 ZIP 文件

4. **解压使用**
   ```
   rustdesk-config-installer-v1.4.0.zip
   ├── RustDesk_Config_Installer.exe
   ├── CHECKSUMS.txt
   └── RELEASE_NOTES.md
   ```

### 方法 2: 使用 GitHub CLI

```bash
# 列出所有 artifacts
gh run list --workflow=build-config-installer.yml

# 下载最新构建的 artifacts
gh run download

# 下载特定运行的 artifacts
gh run download <run-id>

# 下载到指定目录
gh run download --dir ./downloads
```

### 方法 3: 使用 PowerShell 脚本

创建 `Download-LatestInstaller.ps1`:

```powershell
$repo = "CislinkNL/rustdesk"
$workflow = "build-config-installer.yml"
$token = $env:GITHUB_TOKEN  # 设置环境变量

# 获取最新成功的运行
$runsUrl = "https://api.github.com/repos/$repo/actions/workflows/$workflow/runs?status=success&per_page=1"
$headers = @{
    "Accept" = "application/vnd.github+json"
    "Authorization" = "Bearer $token"
}

$runs = Invoke-RestMethod -Uri $runsUrl -Headers $headers
$latestRun = $runs.workflow_runs[0]

Write-Host "Latest run: $($latestRun.id) - $($latestRun.display_title)"

# 获取 artifacts
$artifactsUrl = $latestRun.artifacts_url
$artifacts = Invoke-RestMethod -Uri $artifactsUrl -Headers $headers

foreach ($artifact in $artifacts.artifacts) {
    Write-Host "Downloading: $($artifact.name)"
    $downloadUrl = $artifact.archive_download_url
    $outputPath = "$($artifact.name).zip"
    
    Invoke-RestMethod -Uri $downloadUrl -Headers $headers -OutFile $outputPath
    Write-Host "Downloaded to: $outputPath"
}
```

运行:
```powershell
$env:GITHUB_TOKEN = "your_token_here"
.\Download-LatestInstaller.ps1
```

---

## 🏷️ 创建 Release

### 自动创建 Release (推荐)

1. **创建并推送 tag**
   ```bash
   # 创建 tag
   git tag -a v1.4.0 -m "RustDesk Config Installer v1.4.0"
   
   # 推送 tag
   git push origin v1.4.0
   ```

2. **自动发布**
   - GitHub Actions 会自动检测到 tag
   - 构建安装程序
   - 创建 GitHub Release
   - 上传所有文件 (EXE, CHECKSUMS, RELEASE_NOTES)

3. **访问 Release**
   ```
   https://github.com/CislinkNL/rustdesk/releases
   ```

### 手动创建 Release

1. **访问 Releases 页面**
   ```
   https://github.com/CislinkNL/rustdesk/releases/new
   ```

2. **填写信息**
   - Tag: `v1.4.0`
   - Title: `RustDesk Config Installer v1.4.0`
   - Description: 粘贴 `RELEASE_NOTES.md` 内容

3. **上传文件**
   - 从 Artifacts 下载的文件
   - 拖放上传

4. **发布**
   - 取消勾选 "Set as a pre-release" (除非是测试版本)
   - 点击 "Publish release"

---

## 🔐 设置 GitHub Token (可选)

如果需要使用 CLI 或 API:

### 创建 Personal Access Token

1. **访问设置页面**
   ```
   https://github.com/settings/tokens
   ```

2. **创建新 Token**
   - "Generate new token" → "Generate new token (classic)"
   - Note: `RustDesk Actions`
   - Expiration: 选择有效期
   - **Scopes** (权限):
     - ✅ `repo` (完整仓库访问)
     - ✅ `workflow` (更新 GitHub Actions 工作流)
   - 点击 "Generate token"

3. **保存 Token**
   ```bash
   # Linux/Mac
   export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"
   
   # Windows PowerShell
   $env:GITHUB_TOKEN = "ghp_xxxxxxxxxxxx"
   
   # Windows CMD
   set GITHUB_TOKEN=ghp_xxxxxxxxxxxx
   ```

4. **永久保存** (可选)
   ```bash
   # Linux/Mac (~/.bashrc or ~/.zshrc)
   echo 'export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"' >> ~/.bashrc
   
   # Windows (环境变量)
   setx GITHUB_TOKEN "ghp_xxxxxxxxxxxx"
   ```

---

## 📊 工作流详细说明

### 构建步骤

1. **Checkout Repository**
   - 拉取代码

2. **Setup Inno Setup**
   - 下载并安装 Inno Setup 6

3. **Update Public Key** (可选)
   - 如果提供了新 Public Key,更新所有配置文件

4. **Display Current Configuration**
   - 显示当前配置内容

5. **Build Config Installer**
   - 使用 Inno Setup 编译安装程序

6. **Calculate Checksums**
   - 计算 SHA256 和 MD5 校验和

7. **Create Release Notes**
   - 生成发布说明

8. **Upload Artifacts**
   - 上传构建产物 (保留 90 天)

9. **Upload to Release** (如果是 tag)
   - 自动创建 GitHub Release

### 输出文件

```
Output/
├── RustDesk_Config_Installer.exe    (安装程序)
├── CHECKSUMS.txt                     (校验和)
└── RELEASE_NOTES.md                  (发布说明)
```

---

## 🐛 故障排除

### 问题 1: 工作流未触发

**检查:**
```bash
# 确认文件在触发路径中
git status

# 确认分支正确
git branch

# 手动触发
gh workflow run build-config-installer.yml
```

### 问题 2: Inno Setup 安装失败

**解决:**
- 检查 GitHub Actions 日志
- 确认下载 URL 正确
- 可能需要更新 Inno Setup 版本

### 问题 3: 编译失败

**检查:**
- `Deploy-Config.iss` 文件语法
- 确认所有引用的文件存在
- 查看详细编译日志

### 问题 4: 无法下载 Artifacts

**原因:**
- Artifacts 保留期限为 90 天
- 需要登录 GitHub

**解决:**
```bash
# 使用 GitHub CLI
gh auth login
gh run download
```

### 问题 5: Release 未自动创建

**检查:**
```bash
# 确认 tag 已推送
git ls-remote --tags origin

# 确认 tag 格式正确 (v开头)
git tag

# 手动推送
git push origin v1.4.0
```

---

## 📝 示例工作流程

### 场景 1: 服务器 Docker 重新部署后更新客户端配置

```bash
# 1. Docker 部署完成后获取新 Public Key
docker logs hbbs 2>&1 | grep "Key:"
# 输出: Key: AbCdEf1234567890...

# 2. 在 GitHub 手动触发构建
gh workflow run build-config-installer.yml \
  -f public_key="AbCdEf1234567890..." \
  -f version="1.4.1"

# 3. 等待构建完成
gh run watch

# 4. 下载安装程序
gh run download

# 5. 部署到客户端
# 使用下载的 RustDesk_Config_Installer.exe
```

### 场景 2: 定期更新配置

```bash
# 1. 本地更新配置文件
notepad RustDesk2.toml

# 2. 提交并推送
git add RustDesk2.toml
git commit -m "Update server configuration"
git push

# 3. 自动触发构建
# GitHub Actions 会自动运行

# 4. 从 Artifacts 下载新安装程序
# 访问: https://github.com/CislinkNL/rustdesk/actions
```

### 场景 3: 创建正式版本

```bash
# 1. 确认配置正确
cat RustDesk2.toml

# 2. 创建并推送 tag
git tag -a v1.5.0 -m "Production release v1.5.0"
git push origin v1.5.0

# 3. 自动创建 Release
# GitHub Actions 会自动:
#   - 构建安装程序
#   - 创建 GitHub Release
#   - 上传所有文件

# 4. 访问 Release 页面
# https://github.com/CislinkNL/rustdesk/releases/latest
```

---

## 🔗 相关链接

- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [GitHub CLI 文档](https://cli.github.com/manual/)
- [Inno Setup 文档](https://jrsoftware.org/ishelp/)
- [RustDesk Server Docker 部署指南](DOCKER_DEPLOYMENT_GUIDE.md)

---

**版本**: 1.0  
**日期**: 2025-10-12  
**状态**: ✅ 生产就绪
