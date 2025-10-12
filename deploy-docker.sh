#!/bin/bash
# RustDesk Docker 部署脚本

set -e

echo "========================================="
echo "RustDesk Docker 部署"
echo "========================================="

# 1. 备份旧配置（如果存在）
if [ -d "/root/.config/rustdesk" ]; then
    echo "备份旧配置..."
    cp -r /root/.config/rustdesk /root/.config/rustdesk.backup.$(date +%Y%m%d_%H%M%S)
fi

# 2. 停止现有的 systemd 服务
echo "停止现有 RustDesk systemd 服务..."
systemctl stop rustdesk-hbbs rustdesk-hbbr 2>/dev/null || true
systemctl disable rustdesk-hbbs rustdesk-hbbr 2>/dev/null || true

# 3. 删除 systemd 服务文件
echo "删除 systemd 服务文件..."
rm -f /etc/systemd/system/rustdesk-hbbs.service
rm -f /etc/systemd/system/rustdesk-hbbr.service
systemctl daemon-reload

# 4. 停止旧的进程（但不影响其他服务）
echo "清理旧的 RustDesk 进程..."
pkill -9 hbbs || true
pkill -9 hbbr || true

# 5. 停止旧的 Docker 容器（如果存在）
echo "清理旧的 RustDesk Docker 容器..."
docker stop hbbs hbbr 2>/dev/null || true
docker rm hbbs hbbr 2>/dev/null || true

# 6. 清理旧数据（可选 - 如果需要全新开始）
# echo "清理旧数据..."
# rm -rf /root/.config/rustdesk

echo "✓ 旧服务清理完成（n8n 等其他服务未受影响）"

# 3. 安装 Docker（如果未安装）
if ! command -v docker &> /dev/null; then
    echo "安装 Docker..."
    curl -fsSL https://get.docker.com -o get-docker.sh
    sh get-docker.sh
    rm get-docker.sh
fi

# 4. 安装 Docker Compose（如果未安装）
if ! command -v docker-compose &> /dev/null; then
    echo "安装 Docker Compose..."
    curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
    chmod +x /usr/local/bin/docker-compose
fi

# 7. 创建工作目录
WORK_DIR="/opt/rustdesk"
echo "创建工作目录: $WORK_DIR"
mkdir -p $WORK_DIR/data

# 检查是否有其他 Docker 服务在运行
echo "检查现有 Docker 服务..."
RUNNING_CONTAINERS=$(docker ps --format '{{.Names}}' | grep -v -E '^(hbbs|hbbr)$' || true)
if [ ! -z "$RUNNING_CONTAINERS" ]; then
    echo "✓ 发现其他运行中的容器，将保持不变:"
    echo "$RUNNING_CONTAINERS"
fi

# 8. 创建 docker-compose.yml
cat > $WORK_DIR/docker-compose.yml <<'EOF'
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
      - 21115:21115
      - 21116:21116
      - 21116:21116/udp
      - 21118:21118
    restart: unless-stopped
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  hbbr:
    container_name: hbbr
    image: rustdesk/rustdesk-server:latest
    command: hbbr
    volumes:
      - ./data:/root
    networks:
      - rustdesk-net
    ports:
      - 21117:21117
      - 21119:21119
    restart: unless-stopped
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
EOF

# 9. 启动服务
cd $WORK_DIR
echo "拉取最新镜像..."
docker-compose pull

echo "启动 RustDesk 服务..."
docker-compose up -d

# 10. 等待服务启动
echo "等待服务启动..."
sleep 5

# 11. 显示服务状态
echo ""
echo "========================================="
echo "RustDesk 服务状态："
docker-compose ps

# 12. 显示所有 Docker 容器（确认 n8n 等其他服务未受影响）
echo ""
echo "所有运行中的 Docker 容器："
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# 13. 显示公钥
echo ""
echo "========================================="
echo "服务器公钥："
if [ -f "$WORK_DIR/data/id_ed25519.pub" ]; then
    cat "$WORK_DIR/data/id_ed25519.pub"
else
    echo "等待密钥生成..."
    sleep 3
    cat "$WORK_DIR/data/id_ed25519.pub"
fi

# 14. 显示日志命令
echo ""
echo "========================================="
echo "常用命令："
echo "查看日志: docker-compose -f $WORK_DIR/docker-compose.yml logs -f"
echo "重启服务: docker-compose -f $WORK_DIR/docker-compose.yml restart"
echo "停止服务: docker-compose -f $WORK_DIR/docker-compose.yml down"
echo ""
echo "注意: 此部署只操作 RustDesk 服务，n8n 等其他服务未受影响"
echo "========================================="
