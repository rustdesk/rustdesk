#!/bin/bash
# 从 SSH ED25519 公钥提取 RustDesk 格式的 Key

echo "=== RustDesk Key 格式转换 ==="
echo ""

SSH_KEY_FILE="/opt/rustdesk/data/id_ed25519.pub"

if [ -f "$SSH_KEY_FILE" ]; then
    echo "找到 SSH 格式的 key: $SSH_KEY_FILE"
    echo "内容:"
    cat "$SSH_KEY_FILE"
    echo ""
    echo ""
    
    # 提取 SSH key 的 base64 部分 (第二个字段)
    SSH_KEY_B64=$(awk '{print $2}' "$SSH_KEY_FILE")
    echo "SSH Key Base64 部分:"
    echo "$SSH_KEY_B64"
    echo ""
    echo ""
    
    # 解码 SSH 格式的 key 并提取实际的公钥
    # SSH ED25519 格式: 4字节长度 + "ssh-ed25519" + 4字节长度 + 32字节公钥
    # 我们需要提取最后 32 字节并重新编码为 base64
    
    echo "尝试转换为 RustDesk 格式..."
    RUSTDESK_KEY=$(echo "$SSH_KEY_B64" | base64 -d | tail -c 32 | base64 | tr -d '\n')
    echo "RustDesk 格式 Key:"
    echo "$RUSTDESK_KEY"
    echo ""
    echo ""
    
    # 另一种方法:使用 ssh-keygen 转换
    echo "或者,RustDesk 可能直接使用 SSH key 的 base64 部分:"
    echo "$SSH_KEY_B64"
else
    echo "未找到文件: $SSH_KEY_FILE"
fi

echo ""
echo "=== 检查 RustDesk 服务实际使用的目录 ==="
docker ps --no-trunc | grep hbbs
echo ""
docker inspect $(docker ps -q --filter "name=hbbs") 2>/dev/null | grep -A 5 "Mounts"
