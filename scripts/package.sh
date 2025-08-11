#!/bin/bash
# scripts/package.sh - Build and package freight for distribution

set -euo pipefail

VERSION="0.1.0"
TARGET="${1:-x86_64-unknown-linux-gnu}"
PACKAGE_NAME="freight-${VERSION}-${TARGET}"
BUILD_DIR="target/${TARGET}/release"
PACKAGE_DIR="dist/${PACKAGE_NAME}"

echo "Building and packaging freight ${VERSION} for ${TARGET}..."

# 1. Build Rust executable for target platform
echo "Building Rust executable for ${TARGET}..."
if [[ "$TARGET" != "$(rustc -vV | grep host | cut -d' ' -f2)" ]]; then
    echo "Cross-compiling for ${TARGET}..."
    rustup target add "$TARGET"
    cargo build --release --target "$TARGET"
else
    echo "Building for native target..."
    cargo build --release
    BUILD_DIR="target/release"
fi

# 2. Create package directory structure
echo "Creating package structure..."
rm -rf "dist"
mkdir -p "${PACKAGE_DIR}/bin"
mkdir -p "${PACKAGE_DIR}/lib"

# 3. Copy Rust executable
echo "Copying freight executable..."
cp "${BUILD_DIR}/freight" "${PACKAGE_DIR}/bin/"

# 4. Copy bash tools and make executable
echo "Copying bash tools..."
cp bin/* "${PACKAGE_DIR}/bin/"
chmod +x "${PACKAGE_DIR}/bin"/*

# 5. Copy library scripts
echo "Copying library scripts..."
cp lib/* "${PACKAGE_DIR}/lib/"

# 6. Create installation script
cat > "${PACKAGE_DIR}/install.sh" << 'EOF'
#!/bin/bash
set -euo pipefail

INSTALL_PREFIX="${1:-/usr/local}"
INSTALL_BIN="${INSTALL_PREFIX}/bin"
INSTALL_LIB="${INSTALL_PREFIX}/lib/freight"

echo "Installing freight to ${INSTALL_PREFIX}..."

# Create directories
sudo mkdir -p "${INSTALL_BIN}"
sudo mkdir -p "${INSTALL_LIB}"

# Install binaries
sudo cp bin/* "${INSTALL_BIN}/"
sudo chmod +x "${INSTALL_BIN}"/freight*

# Install libraries
sudo cp lib/* "${INSTALL_LIB}/"

# Update library paths in scripts
for script in "${INSTALL_BIN}"/freight-*; do
    if [[ -f "$script" && "$script" != "${INSTALL_BIN}/freight" ]]; then
        sudo sed -i "s|LIB_DIR=\"\$(dirname \"\$SCRIPT_DIR\")/lib\"|LIB_DIR=\"${INSTALL_LIB}\"|" "$script"
    fi
done

echo "Installation complete!"
echo "Add ${INSTALL_BIN} to your PATH if not already present."
EOF

chmod +x "${PACKAGE_DIR}/install.sh"

# 7. Create README
cat > "${PACKAGE_DIR}/README.md" << EOF
# Freight NFS Migration Suite v${VERSION}

## Installation

### Option 1: System-wide installation (recommended)
\`\`\`bash
sudo ./install.sh
\`\`\`

### Option 2: User installation
\`\`\`bash
./install.sh ~/.local
echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.bashrc
\`\`\`

### Option 3: Portable usage
Add the bin/ directory to your PATH:
\`\`\`bash
export PATH=\$(pwd)/bin:\$PATH
\`\`\`

## Usage

\`\`\`bash
freight --help
freight init --source /nfs1 --dest /nfs2
freight migrate /nfs1/user1 /nfs2/user1
\`\`\`

## Requirements

- rsync
- bash 4.0+
- Standard Unix utilities (find, du, etc.)
- Optional: jq (for configuration parsing)
EOF

# 8. Create tarball
echo "Creating tarball..."
cd dist
tar -czf "${PACKAGE_NAME}.tar.gz" "${PACKAGE_NAME}"
cd ..

echo "Package created successfully: dist/${PACKAGE_NAME}.tar.gz"
echo ""
echo "Usage:"
echo "  ./scripts/package.sh                           # Build for current platform"
echo "  ./scripts/package.sh x86_64-unknown-linux-gnu  # Cross-compile for Linux x64"
echo "  ./scripts/package.sh x86_64-unknown-linux-musl # Cross-compile for Linux x64 (static)"

# 9. Show package contents
echo ""
echo "Package contents:"
find "dist/${PACKAGE_NAME}" -type f | sort