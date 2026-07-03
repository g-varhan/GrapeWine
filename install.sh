#!/usr/bin/env bash
#
# GrapeWine Universal Installer & Uninstaller Script
# Installs/Uninstalls the game orchestrator and binds the 'grape' / 'grapewine' commands.
#
set -euo pipefail

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# File targets
INSTALL_BIN_DIR="$HOME/.local/bin"
DEST_BIN_GRAPE="$INSTALL_BIN_DIR/grape"
DEST_BIN_GRAPEWINE="$INSTALL_BIN_DIR/grapewine"
DATA_DIR="$HOME/.local/share/grapevine"

# Function to perform uninstallation
uninstall_grapewine() {
    echo -e "${YELLOW}===================================================${NC}"
    echo -e "${YELLOW}         Uninstalling GrapeWine...                 ${NC}"
    echo -e "${YELLOW}===================================================${NC}"

    local removed=0

    if [ -f "$DEST_BIN_GRAPE" ]; then
        echo -e "Removing executable: ${RED}$DEST_BIN_GRAPE${NC}"
        rm -f "$DEST_BIN_GRAPE"
        removed=1
    fi

    if [ -f "$DEST_BIN_GRAPEWINE" ]; then
        echo -e "Removing executable: ${RED}$DEST_BIN_GRAPEWINE${NC}"
        rm -f "$DEST_BIN_GRAPEWINE"
        removed=1
    fi

    if [ -d "$DATA_DIR" ]; then
        echo -e "\n${YELLOW}Found GrapeWine data directory containing prefixes & library database at:${NC}"
        echo -e "  ${BLUE}$DATA_DIR${NC}"
        
        # Read reply (default to NO for safety to prevent losing games)
        read -rp "Would you like to delete all game prefixes and configuration database? [y/N]: " confirm
        if [[ "$confirm" =~ ^[Yy]$ ]]; then
            echo -e "Removing data directory: ${RED}$DATA_DIR${NC}"
            rm -rf "$DATA_DIR"
            echo -e "${GREEN}✓ Data directory successfully deleted.${NC}"
        else
            echo -e "${GREEN}Keeping game prefixes and configuration database intact.${NC}"
        fi
    fi

    if [ $removed -eq 1 ]; then
        echo -e "\n${GREEN}✓ GrapeWine executables successfully uninstalled!${NC}"
    else
        echo -e "\n${YELLOW}No active GrapeWine installations found in $INSTALL_BIN_DIR.${NC}"
    fi
}

# Function to automatically append path to shell config if missing
setup_shell_path() {
    local shell_name
    shell_name=$(basename "${SHELL:-bash}")
    local config_file=""
    local export_line='export PATH="$HOME/.local/bin:$PATH"'

    if [[ ":$PATH:" == *":$HOME/.local/bin:"* ]]; then
        echo -e "${GREEN}✓ $HOME/.local/bin is already in your PATH.${NC}"
        return
    fi

    case "$shell_name" in
        zsh)
            config_file="$HOME/.zshrc"
            ;;
        bash)
            config_file="$HOME/.bashrc"
            ;;
        fish)
            config_file="$HOME/.config/fish/config.fish"
            export_line='fish_add_path ~/.local/bin'
            ;;
        *)
            if [ -f "$HOME/.zshrc" ]; then
                config_file="$HOME/.zshrc"
            elif [ -f "$HOME/.bashrc" ]; then
                config_file="$HOME/.bashrc"
            fi
            ;;
    esac

    if [ -n "$config_file" ]; then
        mkdir -p "$(dirname "$config_file")"
        touch "$config_file"
        
        if ! grep -q "$export_line" "$config_file"; then
            echo -e "\n# Added by GrapeWine installer" >> "$config_file"
            echo "$export_line" >> "$config_file"
            echo -e "${GREEN}✓ Successfully added PATH configuration to $config_file!${NC}"
            echo -e "${YELLOW}Please restart your terminal or run: source $config_file${NC}"
        else
            echo -e "${GREEN}✓ PATH configuration already exists in $config_file.${NC}"
        fi
    else
        echo -e "${YELLOW}Could not auto-detect shell config file. Please manually add $HOME/.local/bin to your PATH.${NC}"
    fi
}

# Parse command line arguments
if [ "${1:-}" = "--uninstall" ] || [ "${1:-}" = "-u" ] || [ "${1:-}" = "uninstall" ]; then
    uninstall_grapewine
    exit 0
fi

# Standard Installation flow
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
mkdir -p "$INSTALL_BIN_DIR"

SRC_BIN="$BUILD_PATH/target/release/grapevine"

echo -e "Copying binary to ${YELLOW}$DEST_BIN_GRAPE${NC} and ${YELLOW}$DEST_BIN_GRAPEWINE${NC}..."
cp "$SRC_BIN" "$DEST_BIN_GRAPE"
cp "$SRC_BIN" "$DEST_BIN_GRAPEWINE"
chmod +x "$DEST_BIN_GRAPE" "$DEST_BIN_GRAPEWINE"

# Configure shell path
setup_shell_path

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
echo -e "\nTo uninstall GrapeWine at any time, run:"
echo -e "  ${YELLOW}grapewine --uninstall${NC}"
echo ""
