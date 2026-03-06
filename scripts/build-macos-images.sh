#!/usr/bin/env bash
#
# build-macos-images.sh - Build Docker images for macOS cross-compilation
#
# Usage:
#   ./scripts/build-macos-images.sh [OPTIONS]
#
# Options:
#   --x86_64      Build only x86_64 image
#   --aarch64     Build only aarch64 image
#   --no-cache    Build without Docker cache
#   --help        Show this help message
#
# After building, you can cross-compile with:
#   cross build --release --target x86_64-apple-darwin
#   cross build --release --target aarch64-apple-darwin
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

BUILD_X86_64=true
BUILD_AARCH64=true
NO_CACHE=""

show_help() {
    sed -n '3,15p' "$0" | sed 's/^#//' | sed 's/^ //'
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --x86_64)
            BUILD_X86_64=true
            BUILD_AARCH64=false
            shift
            ;;
        --aarch64)
            BUILD_X86_64=false
            BUILD_AARCH64=true
            shift
            ;;
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        --help|-h)
            show_help
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

cd "$REPO_ROOT"

if $BUILD_X86_64; then
    log "Building x86_64-apple-darwin image..."
    docker build $NO_CACHE \
        -f docker/Dockerfile.macos-x86_64 \
        -t rjest-macos-x86_64:latest \
        .
    success "Built rjest-macos-x86_64:latest"
fi

if $BUILD_AARCH64; then
    log "Building aarch64-apple-darwin image..."
    docker build $NO_CACHE \
        -f docker/Dockerfile.macos-aarch64 \
        -t rjest-macos-aarch64:latest \
        .
    success "Built rjest-macos-aarch64:latest"
fi

echo ""
log "Done! You can now cross-compile with:"
echo "  cross build --release --target x86_64-apple-darwin"
echo "  cross build --release --target aarch64-apple-darwin"
echo ""
warn "Note: Apple SDK licensing applies. Verify compliance for your use case."
