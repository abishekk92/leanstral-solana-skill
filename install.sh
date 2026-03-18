#!/bin/bash
set -e

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "⚠️  Rust toolchain not found"
    echo ""
    echo "This package requires Rust to build the leanstral binary."
    echo ""
    echo "Install Rust from: https://rustup.rs/"
    echo "Then run: npm rebuild leanstral-solana-skill"
    echo ""
    echo "Alternatively, if you're on macOS ARM64, the pre-built binary may work."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    exit 0  # Exit gracefully so npm install doesn't fail
fi

# Check if binary already exists (from prepack)
if [ -f "bin/leanstral" ] && [ -x "bin/leanstral" ]; then
    echo "✓ Pre-built leanstral binary found"

    # Test if it works on this platform
    if ./bin/leanstral --version &> /dev/null; then
        echo "✓ Binary is compatible with this platform"
        exit 0
    else
        echo "⚠ Pre-built binary is not compatible, rebuilding..."
    fi
fi

# Build the binary
echo "Building leanstral binary..."
cargo build --release

# Copy to bin/
mkdir -p bin
cp target/release/leanstral bin/
chmod +x bin/leanstral

echo "✓ leanstral binary built successfully"
