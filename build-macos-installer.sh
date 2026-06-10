#!/bin/bash
# RustDesk macOS Installer Build Script
# 用于构建和打包 macOS DMG 安装程序

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置变量
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/flutter/build/macos/Build/Products/Release"
DMG_DIR="${SCRIPT_DIR}/dmg_build"
CONFIG_FILE="${SCRIPT_DIR}/RustDesk_Config_Template.toml"
APP_NAME="RustDesk"
VOLUME_NAME="RustDesk Installer"
DMG_NAME="RustDesk-Cislink-Installer"

# 版本信息（从 Cargo.toml 读取）
get_version() {
    grep '^version' "${SCRIPT_DIR}/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/'
}

VERSION=$(get_version)
echo -e "${BLUE}=== RustDesk macOS Installer Builder ===${NC}"
echo -e "${BLUE}Version: ${VERSION}${NC}"
echo ""

# 检查是否在 macOS 上运行
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}错误：此脚本只能在 macOS 上运行！${NC}"
    exit 1
fi

# 检查必需的工具
check_dependencies() {
    echo -e "${YELLOW}检查依赖...${NC}"

    local missing_deps=()

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("Rust (https://rustup.rs/)")
    fi

    # 检查 Flutter
    if ! command -v flutter &> /dev/null; then
        missing_deps+=("Flutter (https://flutter.dev/docs/get-started/install/macos)")
    fi

    # 检查 create-dmg
    if ! command -v create-dmg &> /dev/null; then
        echo -e "${YELLOW}未找到 create-dmg，尝试安装...${NC}"
        if command -v brew &> /dev/null; then
            brew install create-dmg
        else
            missing_deps+=("create-dmg (brew install create-dmg)")
        fi
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}缺少以下依赖：${NC}"
        printf '%s\n' "${missing_deps[@]}"
        exit 1
    fi

    echo -e "${GREEN}✓ 所有依赖已安装${NC}"
}

# 步骤 1: 清理之前的构建
clean_build() {
    echo -e "${YELLOW}步骤 1/5: 清理之前的构建...${NC}"

    rm -rf "${DMG_DIR}"
    rm -f "${SCRIPT_DIR}/${DMG_NAME}.dmg"

    if [ -d "${BUILD_DIR}" ]; then
        echo "  清理 Flutter 构建目录..."
        rm -rf "${BUILD_DIR}"
    fi

    echo -e "${GREEN}✓ 清理完成${NC}"
}

# 步骤 2: 构建 Flutter 应用
build_flutter() {
    echo -e "${YELLOW}步骤 2/5: 构建 Flutter 应用...${NC}"

    cd "${SCRIPT_DIR}/flutter"

    # 获取依赖
    echo "  获取 Flutter 依赖..."
    flutter pub get

    # 构建 macOS 应用
    echo "  构建 macOS Release 版本..."
    flutter build macos --release

    if [ ! -d "${BUILD_DIR}/${APP_NAME}.app" ]; then
        echo -e "${RED}错误：Flutter 构建失败！${NC}"
        exit 1
    fi

    cd "${SCRIPT_DIR}"
    echo -e "${GREEN}✓ Flutter 构建完成${NC}"
}

# 步骤 3: 注入配置文件
inject_config() {
    echo -e "${YELLOW}步骤 3/5: 注入配置文件...${NC}"

    if [ ! -f "${CONFIG_FILE}" ]; then
        echo -e "${YELLOW}警告：未找到配置文件 ${CONFIG_FILE}，跳过配置注入${NC}"
        return
    fi

    # RustDesk 配置文件应放在 Resources 目录
    local config_target="${BUILD_DIR}/${APP_NAME}.app/Contents/Resources/rustdesk.toml"

    echo "  复制配置文件到应用包..."
    cp "${CONFIG_FILE}" "${config_target}"

    echo -e "${GREEN}✓ 配置文件已注入${NC}"
}

# 步骤 4: 代码签名（可选）
sign_app() {
    echo -e "${YELLOW}步骤 4/5: 代码签名（可选）...${NC}"

    # 检查是否有开发者证书
    local cert_name="$1"

    if [ -z "$cert_name" ]; then
        echo -e "${YELLOW}未指定签名证书，跳过签名步骤${NC}"
        echo -e "${YELLOW}提示：如需分发，请使用: $0 --sign \"Developer ID Application: Your Name\"${NC}"
        return
    fi

    echo "  使用证书签名: ${cert_name}"
    codesign --force --deep --sign "${cert_name}" "${BUILD_DIR}/${APP_NAME}.app"

    # 验证签名
    if codesign --verify --deep --strict "${BUILD_DIR}/${APP_NAME}.app"; then
        echo -e "${GREEN}✓ 应用签名成功${NC}"
    else
        echo -e "${RED}警告：应用签名验证失败${NC}"
    fi
}

# 步骤 5: 创建 DMG
create_dmg_package() {
    echo -e "${YELLOW}步骤 5/5: 创建 DMG 安装包...${NC}"

    # 创建临时目录
    mkdir -p "${DMG_DIR}"

    # 复制应用到临时目录
    echo "  准备 DMG 内容..."
    cp -R "${BUILD_DIR}/${APP_NAME}.app" "${DMG_DIR}/"

    # 创建 Applications 快捷方式
    ln -s /Applications "${DMG_DIR}/Applications"

    # 创建自定义背景（可选）
    if [ -f "${SCRIPT_DIR}/dmg_background.png" ]; then
        cp "${SCRIPT_DIR}/dmg_background.png" "${DMG_DIR}/.background.png"
    fi

    # 使用 create-dmg 创建 DMG
    echo "  生成 DMG 文件..."
    create-dmg \
        --volname "${VOLUME_NAME}" \
        --volicon "${BUILD_DIR}/${APP_NAME}.app/Contents/Resources/AppIcon.icns" \
        --window-pos 200 120 \
        --window-size 800 400 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 200 190 \
        --hide-extension "${APP_NAME}.app" \
        --app-drop-link 600 185 \
        --no-internet-enable \
        "${SCRIPT_DIR}/${DMG_NAME}-${VERSION}.dmg" \
        "${DMG_DIR}" 2>/dev/null || {
            # create-dmg 有时会返回非零退出码但实际成功，检查文件是否存在
            if [ ! -f "${SCRIPT_DIR}/${DMG_NAME}-${VERSION}.dmg" ]; then
                echo -e "${RED}DMG 创建失败${NC}"
                exit 1
            fi
        }

    # 清理临时文件
    echo "  清理临时文件..."
    rm -rf "${DMG_DIR}"

    echo -e "${GREEN}✓ DMG 创建完成${NC}"
}

# 显示摘要
show_summary() {
    echo ""
    echo -e "${GREEN}=== 构建完成！ ===${NC}"
    echo -e "  输出文件: ${BLUE}${DMG_NAME}-${VERSION}.dmg${NC}"
    echo -e "  文件位置: ${SCRIPT_DIR}/"

    local dmg_size=$(du -h "${SCRIPT_DIR}/${DMG_NAME}-${VERSION}.dmg" | cut -f1)
    echo -e "  文件大小: ${dmg_size}"
    echo ""
    echo -e "${YELLOW}下一步操作：${NC}"
    echo "  1. 打开 DMG 文件测试安装"
    echo "  2. 在目标 Mac 上测试运行"
    echo "  3. 如需分发，请进行公证 (notarization)"
    echo ""
}

# 显示帮助
show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --sign CERT_NAME    使用指定的开发者证书进行代码签名"
    echo "  --clean-only        仅清理构建文件，不进行构建"
    echo "  --help              显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0                                        # 构建未签名的 DMG"
    echo "  $0 --sign \"Developer ID Application: Your Name\"  # 构建并签名"
    echo ""
}

# 主流程
main() {
    local cert_name=""
    local clean_only=false

    # 解析命令行参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            --sign)
                cert_name="$2"
                shift 2
                ;;
            --clean-only)
                clean_only=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                echo -e "${RED}未知选项: $1${NC}"
                show_help
                exit 1
                ;;
        esac
    done

    check_dependencies

    if [ "$clean_only" = true ]; then
        clean_build
        echo -e "${GREEN}清理完成${NC}"
        exit 0
    fi

    clean_build
    build_flutter
    inject_config
    sign_app "$cert_name"
    create_dmg_package
    show_summary
}

main "$@"
