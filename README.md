# RustDesk 自定义服务器构建指南

本指南说明如何通过 GitHub Actions 构建使用自定义服务器的 RustDesk 客户端。

## 支持的服务器配置

RustDesk 支持以下四个服务器配置：

1. **ID 服务器 (Rendezvous Server)**: 负责设备发现和连接建立
2. **中继服务器 (Relay Server)**: 当直连失败时的数据转发
3. **API 服务器 (API Server)**: 提供 Web API 服务
4. **Key (密钥)**: 用于加密通信和身份验证

## 配置方法

### 方法1: 通过 GitHub Secrets (推荐)

1. 进入您的 GitHub 仓库
2. 点击 **Settings** → **Secrets and variables** → **Actions**
3. 点击 **New repository secret** 添加以下密钥：

```
RENDEZVOUS_SERVER = desk1.godin.com.cn:21116
API_SERVER = http://desk.godin.com.cn:21114
RELAY_SERVER = desk.godin.com.cn:21117
RS_PUB_KEY = dHlen3vC96EW9CU3zmVm8LxkpgWDfetKNpSsl
```

### 方法2: 通过工作流输入

使用 `custom-server-build.yml` 工作流，在手动触发时输入服务器配置：

1. 进入 **Actions** 页面
2. 选择 **Custom Server Build** 工作流
3. 点击 **Run workflow**
4. 填写服务器配置信息

## 可用的工作流

### 1. flutter-build.yml (已更新)
- 支持通过 GitHub Secrets 配置自定义服务器
- 适用于所有平台 (Windows, macOS, Linux, Android)
- 自动构建和发布

### 2. ci.yml (已更新)
- 支持通过 GitHub Secrets 配置自定义服务器
- 主要用于持续集成测试

### 3. custom-server-build.yml (新增)
- 专门用于自定义服务器构建
- 支持通过工作流输入或 Secrets 配置
- 提供更灵活的配置选项

## 使用示例

### 示例1: 使用默认服务器配置
```yaml
# 不设置任何 Secrets，将使用 RustDesk 默认服务器
```

### 示例2: 使用自定义服务器
```yaml
# 在 GitHub Secrets 中设置：
RENDEZVOUS_SERVER: "your-id-server.com:21116"
API_SERVER: "http://your-api-server.com:21114"
RELAY_SERVER: "your-relay-server.com:21117"
RS_PUB_KEY: "your-public-key-here"
```

### 示例3: 通过工作流输入
1. 进入 Actions → Custom Server Build
2. 填写以下信息：
   - **ID Server**: `desk1.godin.com.cn:21116`
   - **API Server**: `http://desk.godin.com.cn:21114`
   - **Relay Server**: `desk.godin.com.cn:21117`
   - **Public Key**: `dHlen3vC96EW9CU3zmVm8LxkpgWDfetKNpSsl`

## 构建产物

构建完成后，您将获得以下文件：

- **Windows**: `rustdesk-custom-server-windows-x86_64/rustdesk.exe`
- **macOS**: `rustdesk-custom-server-macos-{arch}.dmg`
- **Android**: `rustdesk-custom-server-android-{arch}.apk`

## 注意事项

1. **安全性**: 将敏感信息（如 Key）存储在 GitHub Secrets 中
2. **服务器可用性**: 确保所有服务器都在运行且可访问
3. **端口配置**: 确保端口号与服务器端配置一致
4. **Key 匹配**: 确保 Key 与服务器端匹配

## 故障排除

### 常见问题

1. **构建失败**: 检查服务器地址和端口是否正确
2. **连接失败**: 验证服务器是否可访问
3. **认证失败**: 确认 Key 是否正确

### 调试步骤

1. 检查 GitHub Actions 日志
2. 验证服务器配置
3. 测试服务器连接性
4. 确认 Key 有效性

## 技术支持

如果您遇到问题，请：

1. 检查 GitHub Actions 构建日志
2. 验证服务器配置
3. 参考 RustDesk 官方文档
4. 在 GitHub Issues 中报告问题

---

**注意**: 本配置仅用于构建客户端，服务器端需要单独部署和配置。
