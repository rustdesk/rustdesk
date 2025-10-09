# RustDesk Repository Secrets 实现分析报告

## 一、传递流程概览

```
GitHub Secrets → GitHub Actions → 环境变量 → Cargo Build → Rust 编译时常量 → 应用运行时配置
```

## 二、变量清单及功能

| 变量名 | 功能 | 默认值 |
|--------|------|--------|
| **APP_NAME** | 应用显示名称 | "RustDesk" |
| **RENDEZVOUS_SERVER** | ID/中继服务器地址 | "rs-ny.rustdesk.com" |
| **RELAY_SERVER** | 中继服务器地址 | "rs-ny.rustdesk.com" |
| **API_SERVER** | API 服务器地址 | "https://admin.rustdesk.com" |
| **RS_PUB_KEY** | 服务器公钥 | "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=" |
| **DEFAULT_PASSWORD** | 默认 PIN 解锁密码 | "" (空) |

## 三、传递链路详解

### 第一步：GitHub Actions 定义（.github/workflows/flutter-build.yml）
```yaml
env:
  APP_NAME: "${{ secrets.APP_NAME }}"
  RENDEZVOUS_SERVER: "${{ secrets.RENDEZVOUS_SERVER }}"
  RELAY_SERVER: "${{ secrets.RELAY_SERVER }}"
  API_SERVER: "${{ secrets.API_SERVER }}"
  RS_PUB_KEY: "${{ secrets.RS_PUB_KEY }}"
  DEFAULT_PASSWORD: "${{ secrets.DEFAULT_PASSWORD }}"
```
**功能**：从 GitHub Repository Secrets 读取变量并设置为工作流环境变量

### 第二步：构建脚本传递（libs/hbb_common/build.rs）
```rust
fn set_env(key: &str) {
    if let Some(val_os) = env::var_os(key) {
        if let Some(val) = val_os.to_str() {
            println!("cargo:rustc-env={}={}", key, val);
        }
    }
}
```
**功能**：在 Rust 编译前将环境变量转换为 rustc 编译时环境变量

### 第三步：运行时配置应用（libs/hbb_common/src/config.rs）

#### 3.1 应用名称配置
```rust
pub static ref APP_NAME: RwLock<String> = RwLock::new(
    option_env!("APP_NAME").unwrap_or("RustDesk").into()
);
```

#### 3.2 服务器配置
```rust
pub static ref PROD_RENDEZVOUS_SERVER: RwLock<String> = RwLock::new(
    option_env!("RENDEZVOUS_SERVER").unwrap_or("rs-ny.rustdesk.com").into()
);
```

#### 3.3 默认设置映射
```rust
pub static ref DEFAULT_SETTINGS: RwLock<HashMap<String, String>> = {
    let mut map = HashMap::new();
    
    // ID服务器
    map.insert("custom-rendezvous-server".to_string(), 
               option_env!("RENDEZVOUS_SERVER").unwrap_or("rs-ny.rustdesk.com").into());
    
    // 中继服务器
    map.insert("relay-server".to_string(), 
               option_env!("RELAY_SERVER").unwrap_or("rs-ny.rustdesk.com").into());
    
    // API服务器
    map.insert("api-server".to_string(), 
               option_env!("API_SERVER").unwrap_or("https://admin.rustdesk.com").into());
    
    // 公钥
    map.insert("key".to_string(), 
               option_env!("RS_PUB_KEY").unwrap_or("OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=").into());
    
    // 默认PIN密码
    map.insert("unlock_pin".to_string(), 
               option_env!("DEFAULT_PASSWORD").unwrap_or("").into());
};
```

### 第四步：Android 应用名称特殊处理（flutter-build.yml）
```bash
if: env.APP_NAME != ''
run: |
  sed -i "s/<string name=\"app_name\">.*<\/string>/<string name=\"app_name\">${{ env.APP_NAME }}<\/string>/g" \
    ./flutter/android/app/src/main/res/values/strings.xml
```
**功能**：在构建 Android APK 时动态替换 strings.xml 中的应用名称

## 四、核心功能说明

### 1. 私有化部署支持
- 允许用户编译时指定自己的服务器地址
- 支持自定义 ID 服务器、中继服务器、API 服务器
- 配置服务器公钥实现安全连接

### 2. 品牌定制
- **APP_NAME**：可自定义应用显示名称，实现 OEM 品牌化
- 在 Android 平台会替换 strings.xml 中的 app_name

### 3. 安全预配置
- **DEFAULT_PASSWORD**：可预设默认 PIN 码
- 配合其他设置实现：
  - 隐藏连接管理窗口（allow-hide-cm）
  - 隐藏托盘图标（hide-tray）
  - 无人值守自动连接

### 4. 容错机制
- 所有配置都有默认值（unwrap_or）
- 如果 Secrets 未设置，使用官方默认值
- 不会因配置缺失导致编译失败

## 五、应用场景

1. **企业私有化部署**：配置私有服务器地址和公钥
2. **OEM 定制版本**：修改应用名称为企业品牌
3. **无人值守模式**：预设 PIN 码并隐藏 UI 元素
4. **多租户 SaaS**：不同租户使用不同服务器地址

## 六、技术特点

✅ **编译时注入**：通过 `option_env!` 在编译时将配置烧录进二进制
✅ **安全性**：敏感信息不暴露在源代码中，通过 GitHub Secrets 管理
✅ **灵活性**：支持所有构建平台（Windows、macOS、Linux、Android、iOS）
✅ **向后兼容**：保留官方默认值，不影响正常构建流程

## 七、使用建议

1. 在 GitHub Repository Settings → Secrets 中配置所需变量
2. 触发 GitHub Actions 构建时自动应用配置
3. 生成的二进制文件已内置所有自定义配置
4. 用户安装后无需手动配置即可连接到指定服务器

---

**报告生成时间**：2025年10月9日  
**分析版本**：RustDesk v1.4.2
