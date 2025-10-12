# RustDesk Server Docker 部署指南 - Elestio 专用

## 📋 目录
- [准备工作](#准备工作)
- [部署步骤](#部署步骤)
- [获取 Public Key](#获取-public-key)
- [客户端配置](#客户端配置)
- [管理维护](#管理维护)
- [故障排除](#故障排除)

---

## 🎯 准备工作

### 1. 确保 Docker 环境
Elestio 应该已经预装了 Docker 和 Docker Compose。验证:

```bash
docker --version
docker-compose --version
```

### 2. 上传部署文件
将以下文件上传到服务器 (例如 `/root/rustdesk/`):
- `docker-compose.yml` - Docker Compose 配置
- `elestio-docker-deploy.sh` - 自动部署脚本

### 3. SSH 连接到 Elestio 服务器
```bash
ssh root@your-elestio-server.com -p YOUR_SSH_PORT
```

---

## 🚀 部署步骤

### 方法 1: 使用自动部署脚本 (推荐)

```bash
# 1. 创建工作目录
mkdir -p /root/rustdesk
cd /root/rustdesk

# 2. 上传 elestio-docker-deploy.sh 和 docker-compose.yml

# 3. 赋予执行权限
chmod +x elestio-docker-deploy.sh

# 4. 运行部署脚本
./elestio-docker-deploy.sh
```

脚本会自动执行:
- ✅ 停止现有 RustDesk 服务 (原生进程和 Docker)
- ✅ 备份现有密钥
- ✅ 创建 Docker Compose 配置
- ✅ 拉取最新镜像
- ✅ 启动容器
- ✅ 显示 Public Key

### 方法 2: 手动部署

```bash
# 1. 停止现有服务
killall hbbs hbbr || true
docker-compose down || true

# 2. 备份现有密钥
mkdir -p ~/rustdesk-backup
cp -r /root/.config/rustdesk ~/rustdesk-backup/ || true
cp -r /var/lib/rustdesk-server ~/rustdesk-backup/ || true

# 3. 创建工作目录
mkdir -p /root/rustdesk
cd /root/rustdesk

# 4. 创建 docker-compose.yml (见下方配置)

# 5. 拉取镜像
docker pull rustdesk/rustdesk-server:latest

# 6. 启动服务
docker-compose up -d

# 7. 查看日志
docker logs hbbs
docker logs hbbr
```

---

## 📄 Docker Compose 配置

创建 `/root/rustdesk/docker-compose.yml`:

```yaml
version: '3'

networks:
  rustdesk-net:
    external: false

services:
  hbbs:
    container_name: hbbs
    image: rustdesk/rustdesk-server:latest
    command: hbbs -r hbbr.cislink.nl:21117
    volumes:
      - ./data:/root
    networks:
      - rustdesk-net
    ports:
      - 21115:21115  # NAT type test
      - 21116:21116  # TCP hole punching
      - 21116:21116/udp  # Heartbeat
      - 21118:21118  # Web client
    restart: unless-stopped

  hbbr:
    container_name: hbbr
    image: rustdesk/rustdesk-server:latest
    command: hbbr
    volumes:
      - ./data:/root
    networks:
      - rustdesk-net
    ports:
      - 21117:21117  # Relay
      - 21119:21119  # Web client relay
    restart: unless-stopped
```

**配置说明:**
- `command: hbbs -r hbbr.cislink.nl:21117` - hbbs 指定 relay 服务器地址
- `volumes: ./data:/root` - 密钥和配置保存在 `./data` 目录
- `restart: unless-stopped` - 自动重启策略

---

## 🔑 获取 Public Key

### 方法 1: 从 hbbs 日志获取

```bash
docker logs hbbs 2>&1 | grep "Key:"
```

输出示例:
```
[2025-10-12 09:00:00] INFO [src/rendezvous_server.rs:1205] Key: AbCdEf1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ1234=
```

### 方法 2: 从文件读取

```bash
cat /root/rustdesk/data/id_ed25519.pub
```

### 方法 3: 进入容器查看

```bash
docker exec hbbs cat /root/id_ed25519.pub
```

### ⚠️ 重要
- **保存 Public Key** - 客户端配置需要使用
- 格式: Base64 编码字符串,通常以 `=` 结尾
- 长度: 44 字符左右

---

## 💻 客户端配置

### Windows 客户端配置

更新 `RustDesk2.toml`:

```toml
rendezvous_server = 'hbbs.cislink.nl:21116'
nat_type = 1
serial = 0

[options]
relay-server = 'hbbr.cislink.nl'
key = 'YOUR_PUBLIC_KEY_HERE'
custom-rendezvous-server = 'hbbs.cislink.nl'
```

### 使用部署脚本

1. 更新 `Deploy-RustDesk2Config.ps1` 中的 Key:
```powershell
$serverConfig = @{
    Server = "hbbs.cislink.nl"
    ServerPort = "21116"
    Relay = "hbbr.cislink.nl"
    Key = "YOUR_PUBLIC_KEY_HERE"  # ← 更新这里
}
```

2. 重新编译安装程序:
```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "Deploy-Config.iss"
```

3. 批量部署到客户端

---

## 🔧 管理维护

### 容器管理

```bash
# 查看容器状态
docker-compose ps

# 查看实时日志
docker-compose logs -f

# 查看 hbbs 日志
docker logs -f hbbs

# 查看 hbbr 日志
docker logs -f hbbr

# 重启服务
docker-compose restart

# 停止服务
docker-compose down

# 启动服务
docker-compose up -d

# 重新构建并启动
docker-compose up -d --force-recreate
```

### 更新 RustDesk Server

```bash
cd /root/rustdesk

# 1. 拉取最新镜像
docker pull rustdesk/rustdesk-server:latest

# 2. 重启容器
docker-compose down
docker-compose up -d

# 3. 验证版本
docker logs hbbs 2>&1 | head -20
```

### 备份和恢复

#### 备份密钥
```bash
# 备份当前密钥
mkdir -p ~/rustdesk-key-backup
cp /root/rustdesk/data/id_ed25519* ~/rustdesk-key-backup/
```

#### 恢复密钥
```bash
# 停止服务
docker-compose down

# 恢复密钥
cp ~/rustdesk-key-backup/id_ed25519* /root/rustdesk/data/

# 启动服务
docker-compose up -d
```

### 防火墙配置

确保以下端口开放:

```bash
# TCP 端口
ufw allow 21115/tcp  # NAT type test
ufw allow 21116/tcp  # TCP hole punching
ufw allow 21117/tcp  # Relay
ufw allow 21118/tcp  # Web client
ufw allow 21119/tcp  # Web client relay

# UDP 端口
ufw allow 21116/udp  # Heartbeat
```

**Elestio 平台**: 在 Elestio 控制面板中配置防火墙规则

---

## 🐛 故障排除

### 问题 1: 容器无法启动

```bash
# 检查容器状态
docker-compose ps

# 查看错误日志
docker-compose logs

# 检查端口占用
netstat -tulpn | grep -E '2111[5-9]'

# 强制重建
docker-compose down -v
docker-compose up -d --force-recreate
```

### 问题 2: 客户端连接失败 "Key 不匹配"

```bash
# 1. 获取当前 Public Key
docker logs hbbs 2>&1 | grep "Key:"

# 2. 验证密钥文件
cat /root/rustdesk/data/id_ed25519.pub

# 3. 更新客户端配置
# 确保客户端使用正确的 Key
```

### 问题 3: 容器频繁重启

```bash
# 查看详细日志
docker logs hbbs --tail 100
docker logs hbbr --tail 100

# 检查资源使用
docker stats

# 检查磁盘空间
df -h
```

### 问题 4: 端口冲突

```bash
# 检查端口占用
netstat -tulpn | grep -E '2111[5-9]'

# 停止冲突的进程
killall hbbs hbbr

# 或修改 docker-compose.yml 端口映射
# 例如: "31116:21116" 将容器 21116 映射到主机 31116
```

### 问题 5: 无法获取 Public Key

```bash
# 方法 1: 查看完整日志
docker logs hbbs

# 方法 2: 进入容器
docker exec -it hbbs sh
cat /root/id_ed25519.pub

# 方法 3: 从主机读取
cat /root/rustdesk/data/id_ed25519.pub

# 方法 4: 重新生成密钥
docker-compose down
rm -rf /root/rustdesk/data/id_ed25519*
docker-compose up -d
sleep 5
docker logs hbbs 2>&1 | grep "Key:"
```

---

## 📊 监控

### 检查服务健康状态

```bash
# 容器运行状态
docker-compose ps

# 容器资源使用
docker stats hbbs hbbr

# 网络连接
netstat -an | grep -E '2111[5-9]'

# 查看连接数
docker exec hbbs netstat -an | grep ESTABLISHED | wc -l
```

### 日志查看

```bash
# 实时日志
docker-compose logs -f

# 最近 100 行
docker-compose logs --tail=100

# 带时间戳
docker-compose logs -f --timestamps

# 过滤错误
docker logs hbbs 2>&1 | grep -i error
```

---

## 🔐 安全建议

1. **定期备份密钥**
```bash
# 自动备份脚本
cat > /root/backup-rustdesk-keys.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/root/rustdesk-backups/$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"
cp -r /root/rustdesk/data/id_ed25519* "$BACKUP_DIR/"
echo "Backup completed: $BACKUP_DIR"
EOF
chmod +x /root/backup-rustdesk-keys.sh

# 添加到 cron (每天凌晨 2 点)
echo "0 2 * * * /root/backup-rustdesk-keys.sh" | crontab -
```

2. **限制访问**
- 使用防火墙限制只允许必要的 IP 访问
- 在 Elestio 控制面板配置 IP 白名单

3. **定期更新**
```bash
# 每周检查更新
docker pull rustdesk/rustdesk-server:latest
docker-compose up -d
```

4. **监控日志**
```bash
# 检查异常连接
docker logs hbbs 2>&1 | grep -i "error\|fail\|denied"
```

---

## 📚 参考资料

- [RustDesk Server GitHub](https://github.com/rustdesk/rustdesk-server)
- [RustDesk 官方文档](https://rustdesk.com/docs/)
- [Docker Compose 文档](https://docs.docker.com/compose/)
- [Elestio 文档](https://docs.elest.io/)

---

## ✅ 检查清单

部署完成后验证:

- [ ] 容器运行正常: `docker-compose ps`
- [ ] hbbs 日志无错误: `docker logs hbbs`
- [ ] hbbr 日志无错误: `docker logs hbbr`
- [ ] 获取到 Public Key
- [ ] 端口监听正常: `netstat -tulpn | grep -E '2111[5-9]'`
- [ ] 客户端能够连接
- [ ] 客户端不再报 "Key 不匹配"
- [ ] 密钥已备份
- [ ] 防火墙规则已配置

---

## 🆘 需要帮助?

如果遇到问题:
1. 检查容器日志: `docker-compose logs`
2. 查看防火墙设置
3. 验证端口开放: `telnet hbbs.cislink.nl 21116`
4. 检查 DNS 解析: `nslookup hbbs.cislink.nl`

---

**版本**: 1.0  
**日期**: 2025-10-12  
**状态**: ✅ 生产就绪
