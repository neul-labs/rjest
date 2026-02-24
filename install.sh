#!/usr/bin/env bash
#
# install.sh - Install rjest (jest + jestd) binaries
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/neul-labs/rjest/main/install.sh | sh
#
# Environment:
#   INSTALL_PREFIX      Installation directory (default: ~/.local/bin)
#   RJEST_VERSION       Version to install (default: latest)
#   RJEST_SKIP_CHECKSUM Skip SHA256 verification (default: false)
#
# The script will:
#   1. Try to download pre-built binaries from GitHub releases
#   2. Fall back to building from source if run inside the rjest git repo
#

set -euo pipefail

# Configuration
GITHUB_REPO="neul-labs/rjest"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local/bin}"
RJEST_VERSION="${RJEST_VERSION:-}"
RJEST_SKIP_CHECKSUM="${RJEST_SKIP_CHECKSUM:-false}"

# Colors (disabled if not a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Logging functions
log() { echo -e "${BLUE}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }

die() {
    error "$1"
    exit 1
}

# Detect operating system
detect_os() {
    case "$(uname -s)" in
        Linux)   echo "linux" ;;
        Darwin)  echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*|Windows_NT) echo "windows" ;;
        *)       die "Unsupported operating system: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    local os="$1"
    case "$(uname -m)" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64)
            echo "aarch64"
            ;;
        arm64)
            # macOS uses arm64, Linux uses aarch64
            if [[ "$os" == "macos" ]]; then
                echo "arm64"
            else
                echo "aarch64"
            fi
            ;;
        *)
            die "Unsupported architecture: $(uname -m)"
            ;;
    esac
}

# Get binary extension for platform
get_ext() {
    local os="$1"
    if [[ "$os" == "windows" ]]; then
        echo ".exe"
    else
        echo ""
    fi
}

# Get archive extension for platform
get_archive_ext() {
    local os="$1"
    if [[ "$os" == "windows" ]]; then
        echo "zip"
    else
        echo "tar.gz"
    fi
}

# Download a file with curl or wget
download() {
    local url="$1"
    local dest="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$dest" "$url"
    else
        die "Neither curl nor wget found. Please install one of them."
    fi
}

# Fetch latest version from GitHub API
get_latest_version() {
    local url="https://api.github.com/repos/$GITHUB_REPO/releases/latest"
    local response

    if command -v curl >/dev/null 2>&1; then
        response=$(curl -fsSL "$url" 2>/dev/null) || return 1
    elif command -v wget >/dev/null 2>&1; then
        response=$(wget -qO- "$url" 2>/dev/null) || return 1
    else
        return 1
    fi

    # Extract tag_name from JSON (simple grep approach, no jq dependency)
    echo "$response" | grep -o '"tag_name":\s*"[^"]*"' | head -1 | sed 's/.*"tag_name":\s*"\([^"]*\)".*/\1/' | sed 's/^v//'
}

# Verify checksum
verify_checksum() {
    local file="$1"
    local checksums_file="$2"
    local expected_name="$3"

    if [[ "$RJEST_SKIP_CHECKSUM" == "true" ]]; then
        log "Skipping checksum verification"
        return 0
    fi

    if [[ ! -f "$checksums_file" ]]; then
        warn "Checksums file not found, skipping verification"
        return 0
    fi

    local expected actual

    # Extract expected checksum
    expected=$(grep "$expected_name" "$checksums_file" 2>/dev/null | awk '{print $1}')
    if [[ -z "$expected" ]]; then
        warn "Checksum for $expected_name not found in SHASUMS256.txt"
        return 0
    fi

    # Calculate actual checksum
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$file" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$file" | awk '{print $1}')
    else
        warn "No SHA256 tool found, skipping verification"
        return 0
    fi

    if [[ "$expected" != "$actual" ]]; then
        die "Checksum mismatch for $expected_name: expected $expected, got $actual"
    fi

    success "Checksum verified"
}

# Check if we're inside the rjest git repository
is_rjest_repo() {
    local repo_root

    # Check if we're in a git repo
    repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || return 1

    # Check for rjest workspace Cargo.toml
    if [[ -f "$repo_root/Cargo.toml" ]]; then
        if grep -q 'members = \[' "$repo_root/Cargo.toml" && \
           grep -q '"crates/jestd"' "$repo_root/Cargo.toml"; then
            echo "$repo_root"
            return 0
        fi
    fi

    return 1
}

# Build from source
build_from_source() {
    local repo_root="$1"
    local ext="$2"

    log "Building from source..."

    if ! command -v cargo >/dev/null 2>&1; then
        die "cargo not found. Please install Rust: https://rustup.rs"
    fi

    cd "$repo_root"
    cargo build --release

    local target_dir="$repo_root/target/release"

    if [[ ! -f "$target_dir/jest$ext" ]] || [[ ! -f "$target_dir/jestd$ext" ]]; then
        die "Build failed: binaries not found"
    fi

    # Install binaries
    mkdir -p "$INSTALL_PREFIX"
    cp "$target_dir/jest$ext" "$INSTALL_PREFIX/"
    cp "$target_dir/jestd$ext" "$INSTALL_PREFIX/"

    # Make executable (not needed on Windows)
    if [[ "$ext" == "" ]]; then
        chmod +x "$INSTALL_PREFIX/jest"
        chmod +x "$INSTALL_PREFIX/jestd"
    fi

    success "Built and installed from source"
}

# Install from GitHub release
install_from_release() {
    local version="$1"
    local os="$2"
    local arch="$3"
    local ext="$4"
    local archive_ext="$5"

    local archive_name="rjest-${os}-${arch}.${archive_ext}"
    local base_url="https://github.com/$GITHUB_REPO/releases/download/v$version"
    local archive_url="$base_url/$archive_name"
    local checksums_url="$base_url/SHASUMS256.txt"

    log "Downloading $archive_name..."

    # Create temp directory
    local temp_dir
    temp_dir=$(mktemp -d)
    trap "rm -rf '$temp_dir'" EXIT

    local archive_path="$temp_dir/$archive_name"
    local checksums_path="$temp_dir/SHASUMS256.txt"

    # Download archive
    if ! download "$archive_url" "$archive_path"; then
        return 1
    fi

    # Download checksums (optional)
    download "$checksums_url" "$checksums_path" 2>/dev/null || true

    # Verify checksum
    verify_checksum "$archive_path" "$checksums_path" "$archive_name"

    # Extract archive
    log "Extracting..."
    cd "$temp_dir"

    if [[ "$archive_ext" == "zip" ]]; then
        if command -v unzip >/dev/null 2>&1; then
            unzip -q "$archive_name"
        else
            die "unzip not found. Please install it."
        fi
    else
        tar -xzf "$archive_name"
    fi

    # Find extracted directory
    local extract_dir="rjest-${os}-${arch}"
    if [[ ! -d "$extract_dir" ]]; then
        # Try alternative: files might be at root
        extract_dir="."
    fi

    # Install binaries
    mkdir -p "$INSTALL_PREFIX"

    if [[ -f "$extract_dir/jest$ext" ]]; then
        cp "$extract_dir/jest$ext" "$INSTALL_PREFIX/"
    else
        die "jest$ext not found in archive"
    fi

    if [[ -f "$extract_dir/jestd$ext" ]]; then
        cp "$extract_dir/jestd$ext" "$INSTALL_PREFIX/"
    else
        warn "jestd$ext not found in archive (may be a CLI-only release)"
    fi

    # Make executable
    if [[ "$ext" == "" ]]; then
        chmod +x "$INSTALL_PREFIX/jest"
        [[ -f "$INSTALL_PREFIX/jestd" ]] && chmod +x "$INSTALL_PREFIX/jestd"
    fi

    cd - >/dev/null
    success "Installed from release"
}

# Check if install prefix is in PATH
check_path() {
    case ":$PATH:" in
        *":$INSTALL_PREFIX:"*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# Print PATH instructions
print_path_instructions() {
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/bash}")

    echo ""
    warn "Installation directory is not in your PATH"
    echo ""
    echo "Add the following to your shell configuration:"
    echo ""

    case "$shell_name" in
        bash)
            echo "  echo 'export PATH=\"$INSTALL_PREFIX:\$PATH\"' >> ~/.bashrc"
            echo "  source ~/.bashrc"
            ;;
        zsh)
            echo "  echo 'export PATH=\"$INSTALL_PREFIX:\$PATH\"' >> ~/.zshrc"
            echo "  source ~/.zshrc"
            ;;
        fish)
            echo "  set -Ux fish_user_paths $INSTALL_PREFIX \$fish_user_paths"
            ;;
        *)
            echo "  export PATH=\"$INSTALL_PREFIX:\$PATH\""
            ;;
    esac
    echo ""
}

# Main function
main() {
    log "Installing rjest..."

    # Detect platform
    local os arch ext archive_ext
    os=$(detect_os)
    arch=$(detect_arch "$os")
    ext=$(get_ext "$os")
    archive_ext=$(get_archive_ext "$os")

    log "Platform: $os-$arch"
    log "Install prefix: $INSTALL_PREFIX"

    # Determine version
    local version="$RJEST_VERSION"
    if [[ -z "$version" ]]; then
        log "Fetching latest version..."
        version=$(get_latest_version) || true
    fi

    # Try downloading from release first
    local installed=false
    if [[ -n "$version" ]]; then
        log "Version: $version"
        if install_from_release "$version" "$os" "$arch" "$ext" "$archive_ext"; then
            installed=true
        else
            warn "Failed to download release, checking for local build option..."
        fi
    else
        warn "Could not determine latest version, checking for local build option..."
    fi

    # Fallback: build from source if in rjest repo
    if ! $installed; then
        local repo_root
        if repo_root=$(is_rjest_repo); then
            log "Detected rjest repository at $repo_root"
            log "Falling back to local build..."
            build_from_source "$repo_root" "$ext"
            installed=true
        else
            die "Could not download release and not inside rjest repository"
        fi
    fi

    # Verify installation
    if [[ -f "$INSTALL_PREFIX/jest$ext" ]]; then
        success "jest installed to $INSTALL_PREFIX/jest$ext"
    fi
    if [[ -f "$INSTALL_PREFIX/jestd$ext" ]]; then
        success "jestd installed to $INSTALL_PREFIX/jestd$ext"
    fi

    # Check PATH
    if ! check_path; then
        print_path_instructions
    else
        echo ""
        log "Run 'jest --version' to verify the installation"
    fi

    echo ""
    success "Installation complete!"
}

main "$@"
