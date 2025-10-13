#!/bin/bash
# RustDesk 服务器 Key 检查脚本

echo "=== RustDesk Key 诊断脚本 ==="
echo ""

# 检查可能的 RustDesk 数据目录
echo "1. 检查 RustDesk 数据目录..."
POSSIBLE_DIRS=(
    "/var/lib/rustdesk-server"
    "/root/.rustdesk"
    "/opt/rustdesk"
    "/home/rustdesk"
    "."
)

for dir in "${POSSIBLE_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        echo "✓ 找到目录: $dir"
        if [ -f "$dir/id_ed25519.pub" ]; then
            echo "  ✓✓ 找到 RustDesk public key!"
            echo "  Key 内容:"
            cat "$dir/id_ed25519.pub"
            echo ""
        fi
    fi
done

# 查找所有 id_ed25519.pub 文件
echo ""
echo "2. 搜索所有 id_ed25519.pub 文件..."
find / -name "id_ed25519.pub" -type f 2>/dev/null | while read file; do
    echo "找到: $file"
    echo "内容:"
    cat "$file"
    echo "---"
done

# 检查 Docker 容器
echo ""
echo "3. 检查 Docker 容器..."
if command -v docker &> /dev/null; then
    docker ps | grep -i rust
    echo ""
    echo "尝试从 Docker 容器获取 key..."
    CONTAINER=$(docker ps | grep -i hbbs | awk '{print $1}' | head -1)
    if [ -n "$CONTAINER" ]; then
        echo "从容器 $CONTAINER 获取 key:"
        docker exec $CONTAINER cat /root/id_ed25519.pub 2>/dev/null || \
        docker exec $CONTAINER cat /data/id_ed25519.pub 2>/dev/null || \
        docker exec $CONTAINER find / -name "id_ed25519.pub" -exec cat {} \; 2>/dev/null
    fi
fi

# 检查进程
echo ""
echo "4. 检查 RustDesk 进程..."
ps aux | grep -E "hbbs|hbbr" | grep -v grep

echo ""
echo "=== 完成 ==="
