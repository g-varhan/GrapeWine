#!/usr/bin/env bash
#
# GrapeWine Universal Installer Script
# Installs the game orchestrator and binds the 'grape' / 'grapewine' commands.
#
set -euo pipefail

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}===================================================${NC}"
echo -e "${BLUE}          GrapeWine Universal Installer            ${NC}"
echo -e "${BLUE}===================================================${NC}"

# 1. Dependency checks
echo -e "\n${BLUE}[1/4] Checking system dependencies...${NC}"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    if [ -f "$HOME/.cargo/bin/cargo" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo -e "${RED}Error: Rust/Cargo is not installed.${NC}"
        echo -e "Please install Rust first: ${YELLOW}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${NC}"
        exit 1
    fi
fi
echo -e "${GREEN}✓ Rust/Cargo found: $(cargo --version)${NC}"

# Check for Zig
if ! command -v zig &> /dev/null; then
    if [ -f "$HOME/.local/bin/zig" ]; then
        export PATH="$HOME/.local/bin:$PATH"
    else
        echo -e "${RED}Error: Zig compiler is not installed.${NC}"
        echo -e "Please install Zig (>= 0.13.0) via your package manager or download it from ziglang.org.${NC}"
        exit 1
    fi
fi
echo -e "${GREEN}✓ Zig found: $(zig version)${NC}"

# Check for aria2c (recommended)
if ! command -v aria2c &> /dev/null; then
    echo -e "${YELLOW}Warning: 'aria2c' not found on your system.${NC}"
    echo -e "It is highly recommended to install 'aria2' via your package manager to support torrent downloads.${NC}"
else
    echo -e "${GREEN}✓ aria2c found: $(aria2c --version | head -n 1)${NC}"
fi

# 2. Setup Build Directory
echo -e "\n${BLUE}[2/4] Preparing source files...${NC}"
TEMP_BUILD_DIR=""

# Check if we are running inside the repository directory
if [ -f "Cargo.toml" ] && grep -q "grapevine" Cargo.toml 2>/dev/null; then
    echo -e "${GREEN}Running installer inside local repository folder. Building current source...${NC}"
    BUILD_PATH=$(pwd)
else
    TEMP_BUILD_DIR=$(mktemp -d -t grapewine-build-XXXXXX)
    echo -e "Cloning repository from GitHub to temporary directory ${YELLOW}${TEMP_BUILD_DIR}${NC}..."
    git clone https://github.com/g-varhan/GrapeWine.git "$TEMP_BUILD_DIR"
    BUILD_PATH="$TEMP_BUILD_DIR"
fi

# 3. Compiling GrapeWine
echo -e "\n${BLUE}[3/4] Compiling GrapeWine (Cargo + Zig compilation)...${NC}"
cd "$BUILD_PATH"
cargo build --release

# 4. Installing Binary and Symlinks
echo -e "\n${BLUE}[4/4] Installing executable files...${NC}"
INSTALL_BIN_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_BIN_DIR"

SRC_BIN="$BUILD_PATH/target/release/grapevine"
DEST_BIN_GRAPE="$INSTALL_BIN_DIR/grape"
DEST_BIN_GRAPEWINE="$INSTALL_BIN_DIR/grapewine"

echo -e "Copying binary to ${YELLOW}$DEST_BIN_GRAPE${NC} and ${YELLOW}$DEST_BIN_GRAPEWINE${NC}..."
cp "$SRC_BIN" "$DEST_BIN_GRAPE"
cp "$SRC_BIN" "$DEST_BIN_GRAPEWINE"
chmod +x "$DEST_BIN_GRAPE" "$DEST_BIN_GRAPEWINE"

# Clean up temp directory if created
if [ -n "$TEMP_BUILD_DIR" ]; then
    echo -e "Cleaning up temporary directory..."
    rm -rf "$TEMP_BUILD_DIR"
fi

echo -e "\n${GREEN}===================================================${NC}"
echo -e "${GREEN}     GrapeWine Installed Successfully!             ${NC}"
echo -e "${GREEN}===================================================${NC}"
echo -e "You can now start the launcher using either command:"
echo -e "  ${YELLOW}grape${NC}"
echo -e "  or"
echo -e "  ${YELLOW}grapewine${NC}"

# Path check warning
if [[ ":$PATH:" != *":$INSTALL_BIN_DIR:"* ]]; then
    echo -e "\n${YELLOW}Warning: $INSTALL_BIN_DIR is not in your PATH environment variable.${NC}"
    echo -e "Add this to your shell config file (e.g. ~/.bashrc or ~/.zshrc):"
    echo -e "  ${BLUE}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
fi
echo ""
