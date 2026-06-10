#!/bin/bash
# RustDesk macOS DMG 重打包脚本
# 用于将自定义配置注入到现有 DMG 文件中
# 必须在 macOS 上运行

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 配置变量
ORIGINAL_DMG="${SCRIPT_DIR}/rustdesk original/rustdesk-1.4.4-x86_64.dmg"
CONFIG_FILE="${SCRIPT_DIR}/RustDesk_Config_Template.toml"
OUTPUT_DMG="${SCRIPT_DIR}/RustDesk-Cislink-1.4.4-x86_64.dmg"
TEMP_DIR="${SCRIPT_DIR}/dmg_temp"
MOUNT_POINT="/Volumes/RustDesk_Mount_$$"
APP_NAME="RustDesk.app"
VOLUME_NAME="RustDesk Cislink"

echo -e "${BLUE}=== RustDesk macOS DMG 重打包工具 ===${NC}"
echo ""

# 检查是否在 macOS 上运行
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}错误：此脚本只能在 macOS 上运行！${NC}"
    echo ""
    echo "请将以下文件复制到 macOS 电脑上运行："
    echo "  1. 此脚本 (repack-macos-dmg.sh)"
    echo "  2. 原始 DMG 文件 (rustdesk-1.4.4-x86_64.dmg)"
    echo "  3. 配置文件 (RustDesk_Config_Template.toml)"
    exit 1
fi

# 检查原始 DMG 文件
if [ ! -f "$ORIGINAL_DMG" ]; then
    echo -e "${RED}错误：找不到原始 DMG 文件！${NC}"
    echo "预期位置: $ORIGINAL_DMG"
    echo ""
    echo "请确保文件存在，或修改脚本中的 ORIGINAL_DMG 变量"
    exit 1
fi

# 检查配置文件
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}错误：找不到配置文件！${NC}"
    echo "预期位置: $CONFIG_FILE"
    exit 1
fi

echo -e "${GREEN}✓ 找到原始 DMG: ${ORIGINAL_DMG}${NC}"
echo -e "${GREEN}✓ 找到配置文件: ${CONFIG_FILE}${NC}"
echo ""

# 清理函数
cleanup() {
    echo -e "${YELLOW}清理临时文件...${NC}"

    # 卸载 DMG
    if mount | grep -q "$MOUNT_POINT"; then
        hdiutil detach "$MOUNT_POINT" -force 2>/dev/null || true
    fi

    # 删除临时目录
    rm -rf "$TEMP_DIR" 2>/dev/null || true
}

# 出错时清理
trap cleanup EXIT

# 步骤 1: 创建临时目录
echo -e "${YELLOW}步骤 1/5: 准备工作目录...${NC}"
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"
echo -e "${GREEN}✓ 临时目录已创建${NC}"

# 步骤 2: 挂载原始 DMG
echo -e "${YELLOW}步骤 2/5: 挂载原始 DMG...${NC}"
hdiutil attach "$ORIGINAL_DMG" -mountpoint "$MOUNT_POINT" -nobrowse -quiet
echo -e "${GREEN}✓ DMG 已挂载到 $MOUNT_POINT${NC}"

# 查找 .app 文件
APP_PATH=$(find "$MOUNT_POINT" -maxdepth 1 -name "*.app" -type d | head -1)
if [ -z "$APP_PATH" ]; then
    echo -e "${RED}错误：在 DMG 中找不到 .app 文件！${NC}"
    exit 1
fi
APP_NAME=$(basename "$APP_PATH")
echo -e "${GREEN}✓ 找到应用: $APP_NAME${NC}"

# 步骤 3: 复制应用到临时目录
echo -e "${YELLOW}步骤 3/5: 复制应用...${NC}"
cp -R "$APP_PATH" "$TEMP_DIR/"
echo -e "${GREEN}✓ 应用已复制${NC}"

# 卸载原始 DMG
hdiutil detach "$MOUNT_POINT" -quiet
echo -e "${GREEN}✓ 原始 DMG 已卸载${NC}"

# 步骤 4: 注入配置文件
echo -e "${YELLOW}步骤 4/5: 注入配置文件...${NC}"

# RustDesk 配置文件位置 (Resources 目录)
CONFIG_TARGET="$TEMP_DIR/$APP_NAME/Contents/Resources/rustdesk.toml"

# 复制配置文件
cp "$CONFIG_FILE" "$CONFIG_TARGET"
echo -e "${GREEN}✓ 配置文件已注入到: Contents/Resources/rustdesk.toml${NC}"

# 显示配置内容
echo -e "${BLUE}配置内容:${NC}"
cat "$CONFIG_TARGET" | sed 's/^/  /'

# 创建 Applications 快捷方式
ln -s /Applications "$TEMP_DIR/Applications"

# 步骤 5: 创建新的 DMG
echo -e "${YELLOW}步骤 5/5: 创建新的 DMG...${NC}"

# 删除旧的输出文件
rm -f "$OUTPUT_DMG"

# 检查是否安装了 create-dmg
if command -v create-dmg &> /dev/null; then
    echo "  使用 create-dmg 创建美观的 DMG..."

    # 获取应用图标
    ICON_PATH="$TEMP_DIR/$APP_NAME/Contents/Resources/AppIcon.icns"
    if [ ! -f "$ICON_PATH" ]; then
        ICON_PATH="$TEMP_DIR/$APP_NAME/Contents/Resources/icon.icns"
    fi

    create-dmg \
        --volname "$VOLUME_NAME" \
        --window-pos 200 120 \
        --window-size 800 400 \
        --icon-size 100 \
        --icon "$APP_NAME" 200 190 \
        --hide-extension "$APP_NAME" \
        --app-drop-link 600 185 \
        --no-internet-enable \
        "$OUTPUT_DMG" \
        "$TEMP_DIR" 2>/dev/null || {
            # create-dmg 有时返回非零但实际成功
            if [ ! -f "$OUTPUT_DMG" ]; then
                echo -e "${YELLOW}create-dmg 失败，使用 hdiutil 创建基本 DMG...${NC}"
                hdiutil create -volname "$VOLUME_NAME" -srcfolder "$TEMP_DIR" -ov -format UDZO "$OUTPUT_DMG"
            fi
        }
else
    echo "  使用 hdiutil 创建 DMG..."
    hdiutil create -volname "$VOLUME_NAME" -srcfolder "$TEMP_DIR" -ov -format UDZO "$OUTPUT_DMG"
fi

# 验证输出
if [ -f "$OUTPUT_DMG" ]; then
    DMG_SIZE=$(du -h "$OUTPUT_DMG" | cut -f1)
    echo -e "${GREEN}✓ DMG 创建成功！${NC}"
    echo ""
    echo -e "${GREEN}=== 完成！ ===${NC}"
    echo -e "  输出文件: ${BLUE}$OUTPUT_DMG${NC}"
    echo -e "  文件大小: $DMG_SIZE"
    echo ""
    echo -e "${YELLOW}使用说明：${NC}"
    echo "  1. 将 DMG 文件分发给用户"
    echo "  2. 用户双击打开 DMG"
    echo "  3. 将 RustDesk 拖到 Applications 文件夹"
    echo "  4. 从 Applications 启动 RustDesk"
    echo ""
    echo -e "${YELLOW}注意：${NC}"
    echo "  - 此 DMG 未签名，用户首次打开时需要在"系统偏好设置 > 安全与隐私"中允许"
    echo "  - 如需消除警告，请进行代码签名和公证"
else
    echo -e "${RED}错误：DMG 创建失败！${NC}"
    exit 1
fi
