#!/bin/bash
# RustDesk Server Docker 部署脚本 - Elestio 专用
# 用途: 在 Elestio 上使用 Docker Compose 部署 RustDesk 服务器

set -e

echo "=========================================="
echo "  RustDesk Server Docker 部署工具"
echo "  适用于: Elestio 平台"
echo "=========================================="
echo ""

# 配置变量
RELAY_SERVER="hbbr.cislink.nl:21117"
DATA_DIR="./data"
DOCKER_COMPOSE_FILE="docker-compose.yml"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 1. 停止现有的 RustDesk 服务
log_info "停止现有的 RustDesk 服务..."
if pgrep -x "hbbs" > /dev/null; then
    log_warn "发现运行中的 hbbs 进程,正在停止..."
    killall hbbs || true
fi

if pgrep -x "hbbr" > /dev/null; then
    log_warn "发现运行中的 hbbr 进程,正在停止..."
    killall hbbr || true
fi

# 停止现有的 Docker 容器
if [ -f "$DOCKER_COMPOSE_FILE" ]; then
    log_info "停止现有的 Docker 容器..."
    docker-compose down || true
fi

# 2. 备份现有密钥
log_info "备份现有密钥..."
BACKUP_DIR="$HOME/rustdesk-backup-$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

# 备份所有可能的密钥位置
for key_dir in "/root/.config/rustdesk" "/var/lib/rustdesk-server" "/opt/rustdesk/data" "./data"; do
    if [ -d "$key_dir" ] && [ -f "$key_dir/id_ed25519.pub" ]; then
        log_info "备份密钥: $key_dir"
        cp -r "$key_dir" "$BACKUP_DIR/"
    fi
done

log_info "备份完成: $BACKUP_DIR"

# 3. 创建 Docker Compose 配置
log_info "创建 Docker Compose 配置..."
cat > docker-compose.yml << 'EOF'
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
EOF

log_info "Docker Compose 配置已创建"

# 4. 创建数据目录
log_info "创建数据目录..."
mkdir -p "$DATA_DIR"

# 5. 拉取最新镜像
log_info "拉取最新的 RustDesk Server 镜像..."
docker pull rustdesk/rustdesk-server:latest

# 6. 启动服务
log_info "启动 RustDesk Server..."
docker-compose up -d

# 7. 等待服务启动
log_info "等待服务启动..."
sleep 5

# 8. 检查容器状态
log_info "检查容器状态..."
docker-compose ps

# 9. 获取 Public Key
log_info ""
log_info "=========================================="
log_info "等待密钥生成..."
sleep 3

# 从 hbbs 容器日志中提取 Key
log_info "从 hbbs 容器获取 Public Key..."
PUBLIC_KEY=""
for i in {1..10}; do
    PUBLIC_KEY=$(docker logs hbbs 2>&1 | grep -oP 'Key: \K[A-Za-z0-9+/=]+' | tail -1)
    if [ -n "$PUBLIC_KEY" ]; then
        break
    fi
    log_warn "尝试 $i/10: 等待密钥生成..."
    sleep 2
done

if [ -z "$PUBLIC_KEY" ]; then
    log_warn "无法从日志中提取 Public Key,尝试从文件读取..."
    if [ -f "$DATA_DIR/id_ed25519.pub" ]; then
        PUBLIC_KEY=$(cat "$DATA_DIR/id_ed25519.pub")
    fi
fi

echo ""
echo "=========================================="
echo "  🎉 RustDesk Server 部署完成!"
echo "=========================================="
echo ""
echo "📋 服务器信息:"
echo "  Rendezvous Server: hbbs.cislink.nl:21116"
echo "  Relay Server:      hbbr.cislink.nl:21117"
echo ""
if [ -n "$PUBLIC_KEY" ]; then
    echo "🔑 Public Key:"
    echo "  $PUBLIC_KEY"
    echo ""
    echo "  ⚠️  请保存此 Key,客户端配置需要使用!"
else
    echo "⚠️  Public Key 未能自动获取"
    echo "  请运行以下命令手动获取:"
    echo "  docker logs hbbs 2>&1 | grep 'Key:'"
    echo "  或者:"
    echo "  cat ./data/id_ed25519.pub"
fi
echo ""
echo "📊 容器状态:"
docker-compose ps
echo ""
echo "📝 查看日志:"
echo "  hbbs: docker logs -f hbbs"
echo "  hbbr: docker logs -f hbbr"
echo ""
echo "🔄 管理命令:"
echo "  启动: docker-compose up -d"
echo "  停止: docker-compose down"
echo "  重启: docker-compose restart"
echo "  日志: docker-compose logs -f"
echo ""
echo "💾 备份位置: $BACKUP_DIR"
echo "=========================================="
