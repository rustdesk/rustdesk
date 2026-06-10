#!/bin/bash
# ===================================================
# 一键构建Cislink版RustDesk for macOS
# 自动构建带有Cislink服务器配置的DMG安装包
# ===================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${SCRIPT_DIR}/RustDesk_Config_Template.toml"

# 清屏并显示标题
clear
echo -e "${CYAN}"
echo "╔════════════════════════════════════════════════════════╗"
echo "║                                                        ║"
echo "║     一键构建 Cislink 版 RustDesk for macOS             ║"
echo "║                                                        ║"
echo "║     Cislink 远程桌面解决方案                           ║"
echo "║                                                        ║"
echo "╚════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# 步骤 1: 检查运行环境
check_environment() {
    echo -e "${CYAN}[1/5] 检查运行环境...${NC}"

    # 检查是否在 macOS 上
    if [[ "$OSTYPE" != "darwin"* ]]; then
        echo -e "${RED}✗ 错误：此脚本只能在 macOS 上运行！${NC}"
        echo -e "${YELLOW}  如需在 Windows 上构建，请使用 build-installer.ps1${NC}"
        exit 1
    fi
    echo -e "${GREEN}  ✓ 运行环境检查通过${NC}"

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}✗ 未找到 Rust${NC}"
        echo -e "${YELLOW}  请访问 https://rustup.rs/ 安装 Rust${NC}"
        exit 1
    fi
    echo -e "${GREEN}  ✓ Rust 已安装 ($(rustc --version | cut -d' ' -f2))${NC}"

    # 检查 Flutter
    if ! command -v flutter &> /dev/null; then
        echo -e "${RED}✗ 未找到 Flutter${NC}"
        echo -e "${YELLOW}  请访问 https://flutter.dev/docs/get-started/install/macos 安装 Flutter${NC}"
        exit 1
    fi
    echo -e "${GREEN}  ✓ Flutter 已安装 ($(flutter --version | head -1 | cut -d' ' -f2))${NC}"

    # 检查 create-dmg
    if ! command -v create-dmg &> /dev/null; then
        echo -e "${YELLOW}  ⚠ 未找到 create-dmg，正在安装...${NC}"
        if command -v brew &> /dev/null; then
            brew install create-dmg
            echo -e "${GREEN}  ✓ create-dmg 安装成功${NC}"
        else
            echo -e "${RED}✗ 未找到 Homebrew${NC}"
            echo -e "${YELLOW}  请先安装 Homebrew: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"${NC}"
            exit 1
        fi
    else
        echo -e "${GREEN}  ✓ create-dmg 已安装${NC}"
    fi

    echo ""
}

# 步骤 2: 确认配置
confirm_configuration() {
    echo -e "${CYAN}[2/5] 确认Cislink服务器配置...${NC}"

    if [ ! -f "${CONFIG_FILE}" ]; then
        echo -e "${RED}✗ 错误：配置文件不存在！${NC}"
        echo -e "${YELLOW}  预期位置: ${CONFIG_FILE}${NC}"
        exit 1
    fi

    echo -e "${GREEN}  ✓ 配置文件: ${CONFIG_FILE}${NC}"
    echo ""
    echo -e "${BLUE}  当前配置内容：${NC}"
    echo -e "${YELLOW}  ┌────────────────────────────────────────┐${NC}"

    # 读取并显示配置
    while IFS= read -r line; do
        if [[ ! "$line" =~ ^[[:space:]]*# ]] && [[ -n "$line" ]]; then
            echo -e "${YELLOW}  │ ${NC}$line"
        fi
    done < "${CONFIG_FILE}"

    echo -e "${YELLOW}  └────────────────────────────────────────┘${NC}"
    echo ""

    # 询问用户是否继续
    echo -e "${YELLOW}  是否使用以上配置继续构建？ (y/n)${NC}"
    read -r response
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        echo -e "${RED}  用户取消操作${NC}"
        exit 0
    fi
    echo -e "${GREEN}  ✓ 配置确认完成${NC}"
    echo ""
}

# 步骤 3: 构建应用
build_application() {
    echo -e "${CYAN}[3/5] 构建Flutter应用...${NC}"
    echo -e "${YELLOW}  这可能需要几分钟时间，请耐心等待...${NC}"
    echo ""

    cd "${SCRIPT_DIR}"

    # 检查构建脚本
    if [ ! -f "build-macos-installer.sh" ]; then
        echo -e "${RED}✗ 错误：找不到构建脚本 build-macos-installer.sh${NC}"
        exit 1
    fi

    # 赋予执行权限
    chmod +x build-macos-installer.sh

    # 执行构建
    if ./build-macos-installer.sh; then
        echo ""
        echo -e "${GREEN}  ✓ 应用构建成功${NC}"
    else
        echo -e "${RED}✗ 构建失败，请查看上方错误信息${NC}"
        exit 1
    fi
    echo ""
}

# 步骤 4: 查找生成的DMG
find_dmg() {
    echo -e "${CYAN}[4/5] 查找生成的安装包...${NC}"

    local dmg_file=$(ls -t "${SCRIPT_DIR}"/RustDesk*Cislink*Installer*.dmg 2>/dev/null | head -1)

    if [ -z "$dmg_file" ]; then
        echo -e "${RED}✗ 未找到生成的DMG文件${NC}"
        exit 1
    fi

    DMG_PATH="$dmg_file"
    DMG_SIZE=$(du -h "$DMG_PATH" | cut -f1)
    DMG_NAME=$(basename "$DMG_PATH")

    echo -e "${GREEN}  ✓ 找到安装包: ${DMG_NAME}${NC}"
    echo -e "${GREEN}    文件大小: ${DMG_SIZE}${NC}"
    echo ""
}

# 步骤 5: 完成并显示摘要
show_summary() {
    echo -e "${CYAN}[5/5] 构建完成！${NC}"
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                   构建成功完成！                        ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}📦 安装包信息：${NC}"
    echo -e "   文件名称: ${YELLOW}${DMG_NAME}${NC}"
    echo -e "   文件大小: ${YELLOW}${DMG_SIZE}${NC}"
    echo -e "   文件位置: ${YELLOW}${SCRIPT_DIR}/${NC}"
    echo ""
    echo -e "${BLUE}🔧 服务器配置：${NC}"
    echo -e "   Rendezvous服务器: ${YELLOW}hbbs.cislink.nl${NC}"
    echo -e "   中继服务器: ${YELLOW}hbbr.cislink.nl${NC}"
    echo -e "   公钥已预配置 ✓${NC}"
    echo ""
    echo -e "${BLUE}📋 下一步操作：${NC}"
    echo -e "   ${CYAN}1.${NC} 测试安装包："
    echo -e "      ${YELLOW}open \"${DMG_NAME}\"${NC}"
    echo ""
    echo -e "   ${CYAN}2.${NC} 分发到目标Mac设备"
    echo ""
    echo -e "   ${CYAN}3.${NC} 用户安装步骤："
    echo -e "      • 双击打开DMG文件"
    echo -e "      • 拖拽RustDesk.app到Applications文件夹"
    echo -e "      • 从Applications启动应用"
    echo -e "      • 首次运行时在系统设置中授予权限"
    echo ""
    echo -e "${YELLOW}⚠️  重要提示：${NC}"
    echo -e "   • 如需在多台Mac上分发，建议进行代码签名和公证"
    echo -e "   • 未签名的应用在首次运行时需要右键 -> 打开"
    echo -e "   • 详细说明请查看 MACOS_BUILD_INSTRUCTIONS.md"
    echo ""

    # 询问是否打开DMG
    echo -e "${YELLOW}是否立即打开DMG文件测试？ (y/n)${NC}"
    read -r response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        echo -e "${CYAN}正在打开 ${DMG_NAME}...${NC}"
        open "${DMG_PATH}"
    fi
    echo ""
    echo -e "${GREEN}感谢使用Cislink RustDesk构建工具！${NC}"
    echo ""
}

# 错误处理
handle_error() {
    echo ""
    echo -e "${RED}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                   构建过程出错                          ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${YELLOW}请检查上方的错误信息并尝试以下操作：${NC}"
    echo -e "  1. 确保所有依赖已正确安装"
    echo -e "  2. 检查网络连接（构建过程需要下载依赖）"
    echo -e "  3. 查看 MACOS_BUILD_INSTRUCTIONS.md 获取详细帮助"
    echo -e "  4. 运行 ${CYAN}./build-macos-installer.sh --clean-only${NC} 清理后重试"
    echo ""
}

# 设置错误处理
trap handle_error ERR

# 主流程
main() {
    check_environment
    confirm_configuration
    build_application
    find_dmg
    show_summary
}

# 执行主流程
main

exit 0
