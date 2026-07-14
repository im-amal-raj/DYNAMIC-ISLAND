#!/bin/bash

# Setup colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0;60m' # No Color
CLEAR='\033[0m'

echo -e "${BLUE}==============================================${CLEAR}"
echo -e "${BLUE}        Rust Island Installer 🏝️            ${CLEAR}"
echo -e "${BLUE}==============================================${CLEAR}"

# 1. Dependency Checks
echo -e "\n${YELLOW}[1/5] Checking dependencies...${CLEAR}"
deps=("cargo" "curl" "pkg-config")
for dep in "${deps[@]}"; do
    if ! command -v "$dep" &> /dev/null; then
        echo -e "${RED}Error: '$dep' is not installed. Please install it first.${CLEAR}"
        exit 1
    fi
done
echo -e "${GREEN}✓ All basic build dependencies are present.${CLEAR}"

# 2. Download Icons
echo -e "\n${YELLOW}[2/5] Fetching icon assets...${CLEAR}"
if [ -f "./download_icons.sh" ]; then
    chmod +x ./download_icons.sh
    ./download_icons.sh
else
    echo -e "${RED}Error: download_icons.sh not found. Make sure you are in the project root.${CLEAR}"
    exit 1
fi

# 3. Build Project
echo -e "\n${YELLOW}[3/5] Compiling Rust Island (Release mode)...${CLEAR}"
if cargo build --release; then
    echo -e "${GREEN}✓ Compilation successful.${CLEAR}"
else
    echo -e "${RED}Error: Build failed. Please make sure development libraries are installed.${CLEAR}"
    exit 1
fi

# 4. Copy Binary
echo -e "\n${YELLOW}[4/5] Installing binary and launcher shortcut...${CLEAR}"
BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"

mkdir -p "$BIN_DIR"
mkdir -p "$APP_DIR"

cp target/release/rust-island "$BIN_DIR/rust-island"
echo -e "${GREEN}✓ Copied binary to $BIN_DIR/rust-island${CLEAR}"

# Create desktop file
DESKTOP_FILE="$APP_DIR/rust-island.desktop"
cat <<EOF > "$DESKTOP_FILE"
[Desktop Entry]
Name=Rust Island
Comment=Interactive Dynamic Island widget for Linux
Exec=$BIN_DIR/rust-island
Icon=utilities-system-monitor
Terminal=false
Type=Application
Categories=Utility;System;
StartupWMClass=rust-island
EOF

chmod +x "$DESKTOP_FILE"
echo -e "${GREEN}✓ Created desktop entry at $DESKTOP_FILE${CLEAR}"

# 5. Systemd Service Option
echo -e "\n${YELLOW}[5/5] Configuring Autostart Systemd Service...${CLEAR}"
read -p "Would you like to register rust-island as a systemd user service? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    SYSTEMD_DIR="$HOME/.config/systemd/user"
    mkdir -p "$SYSTEMD_DIR"
    
    SERVICE_FILE="$SYSTEMD_DIR/rust-island.service"
    cat <<EOF > "$SERVICE_FILE"
[Unit]
Description=Rust Island Daemon
After=graphical-session.target

[Service]
ExecStart=$BIN_DIR/rust-island
Restart=on-failure
Environment=SLINT_BACKEND=winit

[Install]
WantedBy=graphical-session.target
EOF

    systemctl --user daemon-reload
    systemctl --user enable rust-island.service
    echo -e "${GREEN}✓ Systemd user service created and enabled!${CLEAR}"
    echo -e "${BLUE}To start the service now, run: systemctl --user start rust-island${CLEAR}"
else
    echo -e "${YELLOW}Skipping systemd setup. You can start the daemon manually by running 'rust-island'.${CLEAR}"
fi

echo -e "\n${GREEN}==============================================${CLEAR}"
echo -e "${GREEN}  Installation Complete! Enjoy your Island! 🏝️ ${CLEAR}"
echo -e "${GREEN}==============================================${CLEAR}"
