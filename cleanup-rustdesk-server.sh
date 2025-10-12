#!/bin/bash
# RustDesk Server 完全清理脚本
# 用途: 删除所有现有的 RustDesk 服务 (原生 + Docker)
# 警告: 此脚本会停止所有 RustDesk 服务并可选择性删除数据

set -e

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
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

log_step() {
    echo -e "${CYAN}[STEP]${NC} $1"
}

echo "=========================================="
echo "  RustDesk Server 完全清理工具"
echo "=========================================="
echo ""
log_warn "此脚本将停止并清理所有 RustDesk 服务"
echo ""

# 询问是否备份数据
read -p "是否备份现有数据? (Y/n): " BACKUP_CHOICE
BACKUP_CHOICE=${BACKUP_CHOICE:-Y}

if [[ $BACKUP_CHOICE =~ ^[Yy]$ ]]; then
    BACKUP_DIR="$HOME/rustdesk-backup-$(date +%Y%m%d_%H%M%S)"
    log_info "将备份数据到: $BACKUP_DIR"
    mkdir -p "$BACKUP_DIR"
    DO_BACKUP=true
else
    log_warn "跳过数据备份"
    DO_BACKUP=false
fi

echo ""
echo "=========================================="
echo "  开始清理..."
echo "=========================================="
echo ""

# ============================================
# 1. 停止原生进程
# ============================================
log_step "1. 停止原生 RustDesk 进程..."

if pgrep -x "hbbs" > /dev/null; then
    log_info "发现运行中的 hbbs 进程"
    pgrep -x "hbbs" | while read pid; do
        log_info "  停止 hbbs PID: $pid"
    done
    killall -9 hbbs 2>/dev/null || true
    sleep 2
    log_info "✓ hbbs 进程已停止"
else
    log_info "未发现运行中的 hbbs 进程"
fi

if pgrep -x "hbbr" > /dev/null; then
    log_info "发现运行中的 hbbr 进程"
    pgrep -x "hbbr" | while read pid; do
        log_info "  停止 hbbr PID: $pid"
    done
    killall -9 hbbr 2>/dev/null || true
    sleep 2
    log_info "✓ hbbr 进程已停止"
else
    log_info "未发现运行中的 hbbr 进程"
fi

# ============================================
# 2. 停止并删除 Docker 容器
# ============================================
log_step "2. 停止并删除 Docker 容器..."

if command -v docker &> /dev/null; then
    # 检查 hbbs 容器
    if docker ps -a --format '{{.Names}}' | grep -q '^hbbs$'; then
        log_info "发现 hbbs Docker 容器"
        docker stop hbbs 2>/dev/null || true
        docker rm -f hbbs 2>/dev/null || true
        log_info "✓ hbbs 容器已删除"
    else
        log_info "未发现 hbbs Docker 容器"
    fi
    
    # 检查 hbbr 容器
    if docker ps -a --format '{{.Names}}' | grep -q '^hbbr$'; then
        log_info "发现 hbbr Docker 容器"
        docker stop hbbr 2>/dev/null || true
        docker rm -f hbbr 2>/dev/null || true
        log_info "✓ hbbr 容器已删除"
    else
        log_info "未发现 hbbr Docker 容器"
    fi
    
    # 停止 docker-compose 管理的服务
    if [ -f "docker-compose.yml" ]; then
        log_info "发现 docker-compose.yml,执行清理..."
        docker-compose down -v 2>/dev/null || true
        log_info "✓ Docker Compose 服务已停止"
    fi
    
    # 清理 rustdesk-net 网络
    if docker network ls --format '{{.Name}}' | grep -q 'rustdesk-net'; then
        log_info "删除 rustdesk-net 网络..."
        docker network rm rustdesk-net 2>/dev/null || true
        log_info "✓ Docker 网络已删除"
    fi
else
    log_info "未安装 Docker,跳过容器清理"
fi

# ============================================
# 3. 停止系统服务
# ============================================
log_step "3. 停止系统服务..."

if systemctl list-units --type=service --all | grep -q 'rustdesk'; then
    log_info "发现 RustDesk 系统服务"
    
    for service in hbbs hbbr rustdesk-hbbs rustdesk-hbbr; do
        if systemctl is-active --quiet $service 2>/dev/null; then
            log_info "停止服务: $service"
            systemctl stop $service 2>/dev/null || true
            systemctl disable $service 2>/dev/null || true
        fi
    done
    log_info "✓ 系统服务已停止"
else
    log_info "未发现 RustDesk 系统服务"
fi

# ============================================
# 4. 备份数据
# ============================================
if [ "$DO_BACKUP" = true ]; then
    log_step "4. 备份现有数据..."
    
    BACKUP_COUNT=0
    
    # 备份所有可能的数据位置
    for data_dir in \
        "/root/.config/rustdesk" \
        "/var/lib/rustdesk-server" \
        "/opt/rustdesk/data" \
        "$HOME/.config/rustdesk" \
        "./data"
    do
        if [ -d "$data_dir" ]; then
            log_info "备份: $data_dir"
            cp -r "$data_dir" "$BACKUP_DIR/" 2>/dev/null || true
            BACKUP_COUNT=$((BACKUP_COUNT + 1))
        fi
    done
    
    # 备份配置文件
    for config_file in \
        "/etc/rustdesk-server/hbbs.toml" \
        "/etc/rustdesk-server/hbbr.toml"
    do
        if [ -f "$config_file" ]; then
            log_info "备份配置: $config_file"
            mkdir -p "$BACKUP_DIR/etc/rustdesk-server"
            cp "$config_file" "$BACKUP_DIR/etc/rustdesk-server/" 2>/dev/null || true
            BACKUP_COUNT=$((BACKUP_COUNT + 1))
        fi
    done
    
    if [ $BACKUP_COUNT -gt 0 ]; then
        log_info "✓ 已备份 $BACKUP_COUNT 个位置的数据"
        log_info "✓ 备份位置: $BACKUP_DIR"
    else
        log_warn "未找到需要备份的数据"
    fi
else
    log_info "跳过数据备份"
fi

# ============================================
# 5. 询问是否删除数据
# ============================================
echo ""
log_warn "警告: 以下操作将删除所有 RustDesk 数据和配置文件"
read -p "是否删除所有 RustDesk 数据? (y/N): " DELETE_CHOICE
DELETE_CHOICE=${DELETE_CHOICE:-N}

if [[ $DELETE_CHOICE =~ ^[Yy]$ ]]; then
    log_step "5. 删除 RustDesk 数据..."
    
    # 删除所有数据目录
    for data_dir in \
        "/root/.config/rustdesk" \
        "/var/lib/rustdesk-server" \
        "/opt/rustdesk/data" \
        "/opt/rustdesk" \
        "$HOME/.config/rustdesk" \
        "./data"
    do
        if [ -d "$data_dir" ]; then
            log_info "删除: $data_dir"
            rm -rf "$data_dir"
        fi
    done
    
    # 删除配置文件
    if [ -d "/etc/rustdesk-server" ]; then
        log_info "删除: /etc/rustdesk-server"
        rm -rf "/etc/rustdesk-server"
    fi
    
    # 删除可执行文件
    for bin_file in /usr/bin/hbbs /usr/bin/hbbr /usr/local/bin/hbbs /usr/local/bin/hbbr; do
        if [ -f "$bin_file" ]; then
            log_info "删除: $bin_file"
            rm -f "$bin_file"
        fi
    done
    
    # 删除系统服务文件
    for service_file in \
        "/etc/systemd/system/hbbs.service" \
        "/etc/systemd/system/hbbr.service" \
        "/etc/systemd/system/rustdesk-hbbs.service" \
        "/etc/systemd/system/rustdesk-hbbr.service"
    do
        if [ -f "$service_file" ]; then
            log_info "删除服务文件: $service_file"
            rm -f "$service_file"
        fi
    done
    
    # 重新加载 systemd
    if command -v systemctl &> /dev/null; then
        systemctl daemon-reload
    fi
    
    log_info "✓ 数据删除完成"
else
    log_warn "保留现有数据(未删除)"
fi

# ============================================
# 6. 清理 Docker 镜像 (可选)
# ============================================
echo ""
read -p "是否删除 RustDesk Docker 镜像? (y/N): " DELETE_IMAGE
DELETE_IMAGE=${DELETE_IMAGE:-N}

if [[ $DELETE_IMAGE =~ ^[Yy]$ ]] && command -v docker &> /dev/null; then
    log_step "6. 清理 Docker 镜像..."
    
    if docker images | grep -q 'rustdesk/rustdesk-server'; then
        log_info "删除 RustDesk Server 镜像..."
        docker rmi $(docker images 'rustdesk/rustdesk-server' -q) 2>/dev/null || true
        log_info "✓ Docker 镜像已删除"
    else
        log_info "未发现 RustDesk Docker 镜像"
    fi
fi

# ============================================
# 7. 验证清理结果
# ============================================
echo ""
log_step "7. 验证清理结果..."

ISSUES_FOUND=false

# 检查进程
if pgrep -x "hbbs" > /dev/null || pgrep -x "hbbr" > /dev/null; then
    log_error "✗ 仍有 RustDesk 进程在运行"
    ps aux | grep -E 'hbb[sr]' | grep -v grep
    ISSUES_FOUND=true
else
    log_info "✓ 无 RustDesk 进程运行"
fi

# 检查 Docker 容器
if command -v docker &> /dev/null; then
    if docker ps -a | grep -E 'hbb[sr]'; then
        log_error "✗ 仍有 RustDesk Docker 容器"
        docker ps -a | grep -E 'hbb[sr]'
        ISSUES_FOUND=true
    else
        log_info "✓ 无 RustDesk Docker 容器"
    fi
fi

# 检查端口占用
if netstat -tuln 2>/dev/null | grep -E ':2111[5-9]' || ss -tuln 2>/dev/null | grep -E ':2111[5-9]'; then
    log_warn "⚠ RustDesk 端口仍被占用"
    netstat -tuln 2>/dev/null | grep -E ':2111[5-9]' || ss -tuln 2>/dev/null | grep -E ':2111[5-9]'
    ISSUES_FOUND=true
else
    log_info "✓ RustDesk 端口已释放"
fi

echo ""
echo "=========================================="
if [ "$ISSUES_FOUND" = true ]; then
    log_warn "清理完成,但发现一些残留"
    echo "  请手动检查上述问题"
else
    log_info "✅ 清理完成! 系统已干净"
fi
echo "=========================================="
echo ""

if [ "$DO_BACKUP" = true ]; then
    echo "📦 备份位置: $BACKUP_DIR"
    echo ""
fi

echo "📋 下一步:"
echo "  1. 确认清理结果正常"
echo "  2. 运行 Docker 部署脚本:"
echo "     ./elestio-docker-deploy.sh"
echo ""
echo "🔍 检查命令:"
echo "  进程: ps aux | grep -E 'hbb[sr]'"
echo "  端口: netstat -tuln | grep -E ':2111[5-9]'"
echo "  容器: docker ps -a"
echo ""
