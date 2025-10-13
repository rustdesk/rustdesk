# RustDesk 服务器密钥管理指南

## 🔐 当前密钥信息

**公钥**: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`

**密钥文件位置**:
- 私钥: `/opt/rustdesk/data/id_ed25519`
- 公钥: `/opt/rustdesk/data/id_ed25519.pub`

**创建时间**: 2025年10月12日

---

## ✅ 密钥持久性保证

### Docker 部署的优势

✅ **永久保存**: 密钥存储在宿主机目录 `/opt/rustdesk/data/`  
✅ **容器重启不影响**: 重启 Docker 容器，密钥不变  
✅ **更新不影响**: 更新 RustDesk 镜像，密钥保持不变  
✅ **系统重启不影响**: 服务器重启后，密钥依然存在  

### 数据持久化映射

```yaml
volumes:
  - ./data:/root
```

这个配置确保：
- 容器内的 `/root` 目录映射到宿主机 `/opt/rustdesk/data/`
- 所有配置和密钥文件都保存在宿主机
- 即使删除容器，数据依然保留

---

## 🔄 密钥变化的情况

### ❌ 密钥会变化的情况：

1. **手动删除密钥文件**
   ```bash
   rm /opt/rustdesk/data/id_ed25519*
   ```

2. **完全删除 Docker 卷**
   ```bash
   docker-compose down -v  # -v 参数会删除卷
   rm -rf /opt/rustdesk/data
   ```

3. **重新运行部署脚本时选择清理数据**
   ```bash
   # 如果部署脚本中有这行被取消注释
   rm -rf /root/.config/rustdesk
   ```

### ✅ 密钥不会变化的情况：

✅ 容器重启: `docker-compose restart`  
✅ 容器重建: `docker-compose down && docker-compose up -d`  
✅ 服务器重启: `reboot`  
✅ 更新镜像: `docker-compose pull && docker-compose up -d`  
✅ 修改配置: 编辑 `docker-compose.yml`  

---

## 📋 验证密钥是否变化

### 方法 1: 使用管理脚本

```powershell
.\Deploy-RustDesk-Docker.ps1 -GetKey
```

### 方法 2: SSH 到服务器查看

```bash
cat /opt/rustdesk/data/id_ed25519.pub
```

### 方法 3: 检查文件创建时间

```bash
ls -lh /opt/rustdesk/data/id_ed25519
```

如果创建时间是 `Oct 12 21:56`，说明密钥从未改变。

---

## 🛡️ 密钥备份建议

虽然密钥会持久保存，但建议定期备份：

### 备份方法 1: 手动备份

```powershell
# 从服务器下载密钥文件
& "D:\Program Files\PuTTY\pscp.exe" -i "d:\Rustdesk\cislink.ppk" `
  root@142.132.187.134:/opt/rustdesk/data/id_ed25519* `
  "d:\Rustdesk\backup\"
```

### 备份方法 2: 记录公钥

将公钥保存在安全的地方：
```
VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
```

### 备份方法 3: 定期快照

如果使用云服务器，可以定期创建磁盘快照。

---

## 🔄 如果密钥意外改变了怎么办？

### 影响：
❌ 已安装的客户端无法连接（密钥不匹配）  
❌ 需要重新分发新的安装包  
❌ 现有用户需要重新配置  

### 解决方案：

#### 方案 1: 恢复旧密钥（推荐）

```bash
# 停止服务
cd /opt/rustdesk
docker-compose down

# 恢复备份的密钥
cp /path/to/backup/id_ed25519* ./data/

# 重启服务
docker-compose up -d
```

#### 方案 2: 使用新密钥

1. 获取新公钥
   ```powershell
   .\Deploy-RustDesk-Docker.ps1 -GetKey
   ```

2. 更新打包脚本中的密钥
   - 编辑 `Build-RustDesk-Installer.ps1`
   - 修改 `$ServerKey` 变量

3. 重新打包客户端
   ```powershell
   .\Build-RustDesk-Installer.ps1 -SkipDownload
   ```

4. 分发新的安装包给所有用户

---

## 📊 当前部署状态检查

### 检查密钥文件完整性

```bash
# SSH 到服务器
ssh root@142.132.187.134

# 检查文件存在
ls -lh /opt/rustdesk/data/id_ed25519*

# 查看公钥内容
cat /opt/rustdesk/data/id_ed25519.pub

# 检查文件权限
stat /opt/rustdesk/data/id_ed25519
```

### 正确的输出应该是：

```
-rw-r--r-- 1 root root 88 Oct 12 21:56 /opt/rustdesk/data/id_ed25519
-rw-r--r-- 1 root root 44 Oct 12 21:56 /opt/rustdesk/data/id_ed25519.pub
```

公钥内容：
```
VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
```

---

## 📝 最佳实践总结

### ✅ 推荐做法：

1. **首次部署后立即备份密钥**
2. **记录公钥到安全的地方**（如密码管理器）
3. **定期验证密钥未改变**
4. **服务器快照包含密钥文件**
5. **不要运行清理数据的命令**

### ❌ 避免做法：

1. ❌ 随意删除 `/opt/rustdesk/data/` 目录
2. ❌ 使用 `docker-compose down -v`（会删除卷）
3. ❌ 手动删除密钥文件
4. ❌ 在部署脚本中取消注释数据清理命令

---

## 🎯 结论

使用当前的 Docker 部署方式：

✅ **密钥会永久保存**  
✅ **正常操作不会改变密钥**  
✅ **已分发的客户端安装包永久有效**  
✅ **无需担心密钥丢失**  

只要不主动删除密钥文件或数据目录，**密钥将永远保持不变**！

---

**当前公钥（请妥善保存）**:
```
VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=
```

**验证命令**:
```powershell
.\Deploy-RustDesk-Docker.ps1 -GetKey
```
