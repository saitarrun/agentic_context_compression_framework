#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REPO="saitarrun/headroom_inspired_agentic_compression_framework"
BINARY_NAME="compression-mcp"
INSTALL_DIR="${INSTALL_DIR:-.local/bin}"
GITHUB_API="https://api.github.com/repos/${REPO}"

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s)
    local arch=$(uname -m)

    case "$os" in
        Darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                arm64) echo "aarch64-apple-darwin" ;;
                *) echo "unsupported"; return 1 ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64) echo "x86_64-unknown-linux-gnu" ;;
                aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
                *) echo "unsupported"; return 1 ;;
            esac
            ;;
        *)
            echo "unsupported"
            return 1
            ;;
    esac
}

# Get latest release
get_latest_release() {
    curl -s "${GITHUB_API}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/'
}

# Download binary
download_binary() {
    local release=$1
    local target=$2
    local binary_file="${BINARY_NAME}-${target}"
    local download_url="https://github.com/${REPO}/releases/download/${release}/${binary_file}"

    echo -e "${YELLOW}Downloading ${BINARY_NAME} ${release} for ${target}...${NC}"

    if ! curl -fL --progress-bar "${download_url}" -o "${BINARY_NAME}"; then
        echo -e "${RED}Failed to download binary${NC}"
        return 1
    fi

    chmod +x "${BINARY_NAME}"
    echo -e "${GREEN}✓ Downloaded${NC}"
}

# Verify checksum
verify_checksum() {
    local release=$1
    local checksum_url="https://github.com/${REPO}/releases/download/${release}/SHA256SUMS"

    echo -e "${YELLOW}Verifying checksum...${NC}"

    if ! curl -fL "${checksum_url}" | grep "${BINARY_NAME}-" > SHA256SUMS.tmp 2>/dev/null; then
        echo -e "${YELLOW}⚠ Checksum file not available, skipping verification${NC}"
        rm -f SHA256SUMS.tmp
        return 0
    fi

    if sha256sum -c SHA256SUMS.tmp 2>/dev/null; then
        echo -e "${GREEN}✓ Checksum verified${NC}"
        rm -f SHA256SUMS.tmp
        return 0
    else
        echo -e "${RED}✗ Checksum verification failed${NC}"
        rm -f SHA256SUMS.tmp
        return 1
    fi
}

# Install binary
install_binary() {
    local install_path="${INSTALL_DIR}/${BINARY_NAME}"

    mkdir -p "${INSTALL_DIR}"
    mv "${BINARY_NAME}" "${install_path}"

    echo -e "${GREEN}✓ Installed to ${install_path}${NC}"

    # Check if in PATH
    if ! command -v "${BINARY_NAME}" &> /dev/null; then
        echo -e "${YELLOW}⚠ ${INSTALL_DIR} is not in your PATH${NC}"
        echo -e "${YELLOW}Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\"${NC}"
    fi
}

# Configure Claude Code
configure_claude_code() {
    local config_file="${HOME}/.claude/settings.json"
    local install_path="${INSTALL_DIR}/${BINARY_NAME}"

    echo ""
    read -p "Configure Claude Code settings? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ ! -f "${config_file}" ]; then
            mkdir -p "${HOME}/.claude"
            echo "{}" > "${config_file}"
        fi

        # Add MCP server configuration
        python3 << EOF
import json
import os

config_file = "${config_file}"
install_path = "${install_path}"

with open(config_file, 'r') as f:
    try:
        config = json.load(f)
    except json.JSONDecodeError:
        config = {}

if 'mcpServers' not in config:
    config['mcpServers'] = {}

config['mcpServers']['headroom-compression'] = {
    'command': install_path,
    'disabled': False,
    'alwaysAllow': [
        'headroom_compress',
        'headroom_retrieve',
        'headroom_stats'
    ]
}

with open(config_file, 'w') as f:
    json.dump(config, f, indent=2)

print("✓ Claude Code configured")
EOF
    fi
}

# Main installation flow
main() {
    echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  Headroom Compression MCP Installer    ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
    echo ""

    # Detect platform
    echo -e "${YELLOW}Detecting platform...${NC}"
    TARGET=$(detect_platform)
    if [ "$?" -ne 0 ] || [ "$TARGET" = "unsupported" ]; then
        echo -e "${RED}Unsupported platform${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Detected: $TARGET${NC}"
    echo ""

    # Get latest release
    echo -e "${YELLOW}Fetching latest release...${NC}"
    RELEASE=$(get_latest_release)
    if [ -z "$RELEASE" ]; then
        echo -e "${RED}Failed to fetch latest release${NC}"
        echo -e "${YELLOW}Manual download: https://github.com/${REPO}/releases${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Latest release: $RELEASE${NC}"
    echo ""

    # Create temp directory
    TMPDIR=$(mktemp -d)
    trap "rm -rf $TMPDIR" EXIT
    cd "$TMPDIR"

    # Download
    if ! download_binary "$RELEASE" "$TARGET"; then
        exit 1
    fi

    # Verify
    if ! verify_checksum "$RELEASE"; then
        exit 1
    fi

    # Install
    if ! install_binary; then
        exit 1
    fi
    echo ""

    # Configure Claude Code
    configure_claude_code
    echo ""

    echo -e "${GREEN}✓ Installation complete!${NC}"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Verify installation: ${BINARY_NAME} --help"
    echo "2. Restart Claude Code for MCP changes to take effect"
    echo ""
    echo "For more info: https://github.com/${REPO}"
}

main "$@"
