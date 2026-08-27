#!/bin/bash
set -e

echo "=== CTF Load 代码推送脚本 ==="
echo ""

# 代理（如不需要可注释掉）
export https_proxy=http://127.0.0.1:7897
export http_proxy=http://127.0.0.1:7897
export all_proxy=socks5://127.0.0.1:7897

# 1. 推送 hbb_common 子模块
echo "[1/3] 推送 hbb_common 子模块到 Ryuki233/hbb_common ..."
cd /Users/ryuki/Downloads/rustdesk/libs/hbb_common
git push origin HEAD:refs/heads/ctfload
echo "✅ hbb_common 推送成功（ctfload 分支）"

# 2. 回到主仓库，提交所有改动
echo ""
echo "[2/3] 提交主仓库改动 ..."
cd /Users/ryuki/Downloads/rustdesk
git add -A
git commit -m "rebrand: RustDesk → CTF Load

- APP_NAME = CTF Load
- Binary: ctfload / ctfload.exe
- Relay: 192.168.31.2 (LAN NAS)
- Bundle ID: com.ctfload.app
- Updated CI workflow paths"
echo "✅ 主仓库提交成功"

# 3. 推送到 GitHub
echo ""
echo "[3/3] 推送到 GitHub ..."
git push origin ctfload-rebrand
echo ""
echo "========================================="
echo "✅ 全部推送完成！"
echo ""
echo "下一步："
echo "  1. 打开 https://github.com/Ryuki233/rustdesk/actions"
echo "  2. 找到 'Build the flutter version' workflow"
echo "  3. 点 'Run workflow' → 选 ctfload-rebrand 分支 → 运行"
echo "========================================="
