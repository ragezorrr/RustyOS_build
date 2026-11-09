#!/usr/bin/env bash
#
# Aqua Compositor — Automated Installer & Updater for Arch Linux
# Repository: https://github.com/ragezorrr/RustyOS
#

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}"
echo "    ___                     "
echo "   /   | ____ ___  ______ _ "
echo "  / /| |/ __ \`/ / / / __ \`/"
echo " / ___ / /_/ / /_/ / /_/ /  "
echo "/_/  |_\__, /\__,_/\__,_/   "
echo "         /_/                "
echo -e "${NC}"
echo -e "${BLUE}==>${NC} Installing Aqua (macOS Sequoia-styled Wayland Compositor in Rust)..."

# 1. Check for Arch Linux package manager (pacman)
if command -v pacman >/dev/null 2>&1; then
    echo -e "${BLUE}==>${NC} Checking system dependencies via pacman..."
    MISSING_PKGS=()
    for pkg in seatd libinput mesa rust base-devel; do
        if ! pacman -Q "$pkg" >/dev/null 2>&1; then
            MISSING_PKGS+=("$pkg")
        fi
    done

    if [ ${#MISSING_PKGS[@]} -gt 0 ]; then
        echo -e "${YELLOW}==>${NC} Installing missing dependencies: ${MISSING_PKGS[*]}"
        sudo pacman -S --needed --noconfirm "${MISSING_PKGS[@]}"
    fi
else
    echo -e "${YELLOW}==>${NC} Not on Arch Linux? Ensure seatd, libinput, and mesa-gbm are installed."
fi

# 2. Check for Cargo / Rust
if ! command -v cargo >/dev/null 2>&1; then
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    else
        echo -e "${RED}==>${NC} Rust / Cargo is not found in PATH. Please install rustup or rust package."
        exit 1
    fi
fi

# 3. Determine source location
INSTALL_SOURCE_DIR="$HOME/.local/share/aqua-source"

if [ -f "$PWD/Cargo.toml" ] && grep -q 'name = "aqua"' "$PWD/Cargo.toml"; then
    echo -e "${BLUE}==>${NC} Installing from current directory ($PWD)..."
    SRC_DIR="$PWD"
else
    echo -e "${BLUE}==>${NC} Cloning / updating source in $INSTALL_SOURCE_DIR..."
    if [ -d "$INSTALL_SOURCE_DIR/.git" ]; then
        cd "$INSTALL_SOURCE_DIR"
        git pull origin main
    else
        mkdir -p "$(dirname "$INSTALL_SOURCE_DIR")"
        git clone https://github.com/ragezorrr/RustyOS.git "$INSTALL_SOURCE_DIR"
        cd "$INSTALL_SOURCE_DIR"
    fi
    SRC_DIR="$INSTALL_SOURCE_DIR"
fi

cd "$SRC_DIR"

# 4. Build release binary
echo -e "${BLUE}==>${NC} Compiling Aqua in release mode (LTO enabled)..."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release

# 5. Install system files
echo -e "${BLUE}==>${NC} Installing binary and session files..."
sudo install -Dm755 target/release/aqua /usr/local/bin/aqua
sudo install -Dm755 aqua-update /usr/local/bin/aqua-update
sudo install -Dm644 aqua.desktop /usr/share/wayland-sessions/aqua.desktop

echo ""
echo -e "${GREEN}================================================================${NC}"
echo -e "${GREEN}      Aqua Compositor has been successfully installed!          ${NC}"
echo -e "${GREEN}================================================================${NC}"
echo ""
echo -e "  • ${YELLOW}Run standalone (from TTY):${NC}  aqua"
echo -e "  • ${YELLOW}Display Manager (SDDM/GDM):${NC} Select 'Aqua (macOS Sequoia)' at login"
echo -e "  • ${YELLOW}Update anytime with:${NC}        aqua-update"
echo ""
