#!/usr/bin/env bash
#
# release.sh - Build and publish rjest releases
#
# Usage:
#   ./scripts/release.sh [OPTIONS]
#
# Options:
#   --dry-run       Show what would be done without executing
#   --skip-build    Use existing binaries (skip cargo build)
#   --target=TARGET Build only for specific target (e.g., linux-x86_64)
#   --all           Build for all supported targets (requires cross for non-native)
#   --draft         Create GitHub release as draft
#   --npm           Publish to npm
#   --pypi          Publish to PyPI
#   --brew          Update Homebrew tap
#   --publish       Publish to all registries (npm, pypi, brew)
#   --skip-github   Skip GitHub release (useful for republishing to registries)
#   --help          Show this help message
#
# Environment:
#   GITHUB_TOKEN      GitHub token for gh CLI (optional if already authenticated)
#   NPM_TOKEN         npm auth token (required for --npm)
#   PYPI_TOKEN        PyPI API token (required for --pypi)
#   HOMEBREW_TAP_REPO Homebrew tap repository (default: neul-labs/homebrew-tap)
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory and repo root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default options
DRY_RUN=false
SKIP_BUILD=false
DRAFT=false
BUILD_ALL=false
SPECIFIC_TARGET=""
PUBLISH_NPM=false
PUBLISH_PYPI=false
PUBLISH_BREW=false
SKIP_GITHUB=false

# Configuration
HOMEBREW_TAP_REPO="${HOMEBREW_TAP_REPO:-neul-labs/homebrew-tap}"
GITHUB_REPO="neul-labs/rjest"

# All supported targets
# Note: windows uses -gnu for cross-compilation compatibility with 'cross'
declare -A TARGET_TRIPLES=(
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-aarch64"]="aarch64-unknown-linux-gnu"
    ["macos-x86_64"]="x86_64-apple-darwin"
    ["macos-arm64"]="aarch64-apple-darwin"
    ["windows-x86_64"]="x86_64-pc-windows-gnu"
)

# Logging functions
log() { echo -e "${BLUE}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }

die() {
    error "$1"
    exit 1
}

# Show help
show_help() {
    sed -n '3,27p' "$0" | sed 's/^#//' | sed 's/^ //'
    exit 0
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --skip-build)
                SKIP_BUILD=true
                shift
                ;;
            --draft)
                DRAFT=true
                shift
                ;;
            --all)
                BUILD_ALL=true
                shift
                ;;
            --target=*)
                SPECIFIC_TARGET="${1#*=}"
                shift
                ;;
            --npm)
                PUBLISH_NPM=true
                shift
                ;;
            --pypi)
                PUBLISH_PYPI=true
                shift
                ;;
            --brew)
                PUBLISH_BREW=true
                shift
                ;;
            --publish)
                PUBLISH_NPM=true
                PUBLISH_PYPI=true
                PUBLISH_BREW=true
                shift
                ;;
            --skip-github)
                SKIP_GITHUB=true
                shift
                ;;
            --help|-h)
                show_help
                ;;
            *)
                die "Unknown option: $1"
                ;;
        esac
    done
}

# Check required dependencies
check_dependencies() {
    log "Checking dependencies..."

    local missing=()

    command -v cargo >/dev/null 2>&1 || missing+=("cargo")
    command -v tar >/dev/null 2>&1 || missing+=("tar")

    # GitHub CLI required unless skipping GitHub
    if ! $SKIP_GITHUB; then
        command -v gh >/dev/null 2>&1 || missing+=("gh")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        die "Missing dependencies: ${missing[*]}"
    fi

    # Check cross is available when building all targets
    if $BUILD_ALL; then
        if ! command -v cross >/dev/null 2>&1; then
            die "cross is required for --all. Install with: cargo install cross --git https://github.com/cross-rs/cross"
        fi
        # Check Docker is running (required for cross)
        if ! docker info >/dev/null 2>&1; then
            die "Docker must be running for cross-compilation"
        fi
        success "cross and Docker available"
    fi

    # Check gh authentication
    if ! $SKIP_GITHUB && ! gh auth status >/dev/null 2>&1; then
        die "GitHub CLI not authenticated. Run 'gh auth login' first."
    fi

    # Check npm if publishing
    if $PUBLISH_NPM; then
        command -v npm >/dev/null 2>&1 || die "npm is required for --npm"
        if [[ -z "${NPM_TOKEN:-}" ]]; then
            warn "NPM_TOKEN not set. Will use npm's default authentication."
        fi
    fi

    # Check pip/twine if publishing to PyPI
    if $PUBLISH_PYPI; then
        command -v python3 >/dev/null 2>&1 || die "python3 is required for --pypi"
        command -v twine >/dev/null 2>&1 || die "twine is required for --pypi. Install with: pip install twine"
        if [[ -z "${PYPI_TOKEN:-}" ]]; then
            warn "PYPI_TOKEN not set. Will use twine's default authentication."
        fi
    fi

    success "All dependencies available"
}

# Extract version from Cargo.toml
get_version() {
    local version
    version=$(grep -m1 '^version = "' "$REPO_ROOT/Cargo.toml" | sed 's/.*version = "\([^"]*\)".*/\1/')
    if [[ -z "$version" ]]; then
        die "Could not extract version from Cargo.toml"
    fi
    echo "$version"
}

# Detect current platform
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux)   os="linux" ;;
        Darwin)  os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)       die "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64)       arch="aarch64" ;;
        arm64)         arch="arm64" ;;
        *)             die "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# Check if we can build for a target natively or need cross
can_build_native() {
    local target="$1"
    local current
    current=$(detect_platform)

    # Normalize arm64 to aarch64 for comparison
    current="${current/arm64/aarch64}"
    local normalized_target="${target/arm64/aarch64}"

    [[ "$current" == "$normalized_target" ]]
}

# Build binaries for a target
build_target() {
    local target="$1"
    local triple="${TARGET_TRIPLES[$target]}"

    if [[ -z "$triple" ]]; then
        die "Unknown target: $target"
    fi

    log "Building for $target ($triple)..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would build: cargo build --release --target $triple"
        return 0
    fi

    local build_cmd="cargo"

    # Use cross for cross-compilation if available and needed
    if ! can_build_native "$target"; then
        if command -v cross >/dev/null 2>&1; then
            build_cmd="cross"
            log "Using cross for cross-compilation"
        else
            warn "Cannot cross-compile to $target without 'cross' tool. Skipping."
            return 1
        fi
    fi

    # Ensure target is installed
    rustup target add "$triple" 2>/dev/null || true

    # Build with static linking for portability
    RUSTFLAGS="-C target-feature=+crt-static" $build_cmd build --release --target "$triple"

    success "Built $target"
}

# Create archive for a target
create_archive() {
    local target="$1"
    local version="$2"
    local staging_dir="$3"
    local triple="${TARGET_TRIPLES[$target]}"
    local ext=""

    [[ "$target" == windows-* ]] && ext=".exe"

    local target_dir="$REPO_ROOT/target/$triple/release"
    local archive_name
    local archive_dir="$staging_dir/rjest-$target"

    # Determine archive format
    if [[ "$target" == windows-* ]]; then
        archive_name="rjest-$target.zip"
    else
        archive_name="rjest-$target.tar.gz"
    fi

    log "Creating archive: $archive_name"

    if $DRY_RUN; then
        log "[DRY-RUN] Would create archive with jest$ext and jestd$ext"
        return 0
    fi

    # Check binaries exist
    if [[ ! -f "$target_dir/jest$ext" ]]; then
        warn "Binary not found: $target_dir/jest$ext"
        return 1
    fi
    if [[ ! -f "$target_dir/jestd$ext" ]]; then
        warn "Binary not found: $target_dir/jestd$ext"
        return 1
    fi

    # Create archive directory
    mkdir -p "$archive_dir"
    cp "$target_dir/jest$ext" "$archive_dir/"
    cp "$target_dir/jestd$ext" "$archive_dir/"

    # Create archive
    cd "$staging_dir"
    if [[ "$target" == windows-* ]]; then
        if command -v zip >/dev/null 2>&1; then
            zip -r "$archive_name" "rjest-$target"
        else
            # Fallback: create tar.gz even for Windows
            warn "zip not found, creating tar.gz instead"
            archive_name="rjest-$target.tar.gz"
            tar -czf "$archive_name" "rjest-$target"
        fi
    else
        tar -czf "$archive_name" "rjest-$target"
    fi
    cd - >/dev/null

    # Cleanup directory, keep archive
    rm -rf "$archive_dir"

    success "Created $archive_name"
}

# Generate checksums
generate_checksums() {
    local staging_dir="$1"

    log "Generating checksums..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would generate SHASUMS256.txt"
        return 0
    fi

    cd "$staging_dir"

    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum rjest-*.tar.gz rjest-*.zip 2>/dev/null > SHASUMS256.txt || true
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 rjest-*.tar.gz rjest-*.zip 2>/dev/null > SHASUMS256.txt || true
    else
        warn "No SHA256 tool found, skipping checksum generation"
        return 1
    fi

    cd - >/dev/null
    success "Generated SHASUMS256.txt"
}

# Create git tag
create_tag() {
    local version="$1"
    local tag="v$version"

    log "Checking tag $tag..."

    if git tag -l "$tag" | grep -q "$tag"; then
        warn "Tag $tag already exists"
        return 0
    fi

    if $DRY_RUN; then
        log "[DRY-RUN] Would create and push tag: $tag"
        return 0
    fi

    log "Creating tag $tag..."
    git tag -a "$tag" -m "Release $tag"
    git push origin "$tag"

    success "Created and pushed tag $tag"
}

# Create GitHub release
create_github_release() {
    local version="$1"
    local staging_dir="$2"
    local tag="v$version"

    log "Creating GitHub release $tag..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would create release with gh release create v$version"
        return 0
    fi

    # Collect release files
    local files=()
    for f in "$staging_dir"/rjest-*.tar.gz "$staging_dir"/rjest-*.zip; do
        [[ -f "$f" ]] && files+=("$f")
    done
    [[ -f "$staging_dir/SHASUMS256.txt" ]] && files+=("$staging_dir/SHASUMS256.txt")

    if [[ ${#files[@]} -eq 0 ]]; then
        die "No release files found"
    fi

    log "Release files:"
    for f in "${files[@]}"; do
        log "  - $(basename "$f")"
    done

    local draft_flag=""
    $DRAFT && draft_flag="--draft"

    # Generate release notes
    local notes="## rjest $tag

### Installation

#### Homebrew (macOS/Linux)
\`\`\`bash
brew install $HOMEBREW_TAP_REPO/rjest
\`\`\`

#### npm
\`\`\`bash
npm install -g @neul-labs/rjest
\`\`\`

#### pip
\`\`\`bash
pip install rjest
\`\`\`

#### Direct download
Download the appropriate archive for your platform below.

### Checksums

See SHASUMS256.txt for SHA256 checksums of all archives.
"

    gh release create "$tag" \
        --title "Release $tag" \
        --notes "$notes" \
        $draft_flag \
        "${files[@]}"

    success "Created GitHub release $tag"
}

# Publish to npm
publish_npm() {
    local version="$1"
    local staging_dir="$2"

    log "Publishing to npm..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would publish to npm"
        return 0
    fi

    local npm_dir="$REPO_ROOT/packages/rjest"
    if [[ ! -d "$npm_dir" ]]; then
        warn "npm package directory not found: $npm_dir"
        return 1
    fi

    cd "$npm_dir"

    # Update version in package.json
    if command -v jq >/dev/null 2>&1; then
        jq ".version = \"$version\"" package.json > package.json.tmp && mv package.json.tmp package.json
    else
        sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" package.json && rm -f package.json.bak
    fi

    # Copy prebuilds if staging dir has them
    if [[ -d "$staging_dir" ]]; then
        mkdir -p prebuilds
        cp "$staging_dir"/rjest-*.tar.gz "$staging_dir"/rjest-*.zip prebuilds/ 2>/dev/null || true
    fi

    # Set npm token if provided
    if [[ -n "${NPM_TOKEN:-}" ]]; then
        echo "//registry.npmjs.org/:_authToken=${NPM_TOKEN}" > .npmrc
    fi

    npm publish --access public

    # Cleanup
    rm -f .npmrc

    cd - >/dev/null
    success "Published to npm"
}

# Publish to PyPI
publish_pypi() {
    local version="$1"
    local staging_dir="$2"

    log "Publishing to PyPI..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would publish to PyPI"
        return 0
    fi

    local pypi_dir="$REPO_ROOT/packages/rjest-python"

    # If python package doesn't exist, create a minimal one
    if [[ ! -d "$pypi_dir" ]]; then
        log "Creating Python package structure..."
        mkdir -p "$pypi_dir/rjest"

        # Create pyproject.toml
        cat > "$pypi_dir/pyproject.toml" << EOF
[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "rjest"
version = "$version"
description = "A fast Jest-compatible test runner written in Rust"
readme = "README.md"
license = {text = "MIT"}
requires-python = ">=3.8"
classifiers = [
    "Development Status :: 4 - Beta",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Operating System :: OS Independent",
    "Programming Language :: Python :: 3",
    "Programming Language :: Rust",
    "Topic :: Software Development :: Testing",
]
keywords = ["jest", "testing", "rust", "runner"]
authors = [
    {name = "Neul Labs", email = "hello@neul.com"}
]

[project.urls]
Homepage = "https://github.com/$GITHUB_REPO"
Repository = "https://github.com/$GITHUB_REPO"
Issues = "https://github.com/$GITHUB_REPO/issues"

[project.scripts]
rjest = "rjest.cli:main"
EOF

        # Create __init__.py
        cat > "$pypi_dir/rjest/__init__.py" << EOF
"""rjest - A fast Jest-compatible test runner written in Rust"""
__version__ = "$version"
EOF

        # Create cli.py
        cat > "$pypi_dir/rjest/cli.py" << 'EOF'
"""CLI wrapper for rjest binary"""
import os
import sys
import platform
import subprocess

def get_binary_path():
    """Get the path to the rjest binary"""
    # Check if binary is in PATH
    binary_name = "jest.exe" if platform.system() == "Windows" else "jest"

    # Try to find in common locations
    locations = [
        os.path.join(os.path.dirname(__file__), "bin", binary_name),
        os.path.join(sys.prefix, "bin", binary_name),
    ]

    for loc in locations:
        if os.path.exists(loc):
            return loc

    # Fall back to PATH
    return binary_name

def main():
    """Run the rjest binary"""
    binary = get_binary_path()
    try:
        result = subprocess.run([binary] + sys.argv[1:])
        sys.exit(result.returncode)
    except FileNotFoundError:
        print("Error: rjest binary not found. Please install the binary separately.", file=sys.stderr)
        print("Visit https://github.com/neul-labs/rjest for installation instructions.", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
EOF

        # Copy README
        cp "$REPO_ROOT/README.md" "$pypi_dir/" 2>/dev/null || echo "# rjest" > "$pypi_dir/README.md"
    else
        # Update version in existing pyproject.toml
        sed -i.bak "s/^version = \"[^\"]*\"/version = \"$version\"/" "$pypi_dir/pyproject.toml" && rm -f "$pypi_dir/pyproject.toml.bak"
    fi

    cd "$pypi_dir"

    # Build the package
    python3 -m build 2>/dev/null || pip install build && python3 -m build

    # Upload to PyPI
    if [[ -n "${PYPI_TOKEN:-}" ]]; then
        twine upload dist/* -u __token__ -p "$PYPI_TOKEN"
    else
        twine upload dist/*
    fi

    cd - >/dev/null
    success "Published to PyPI"
}

# Update Homebrew tap
publish_brew() {
    local version="$1"
    local staging_dir="$2"

    log "Updating Homebrew tap..."

    if $DRY_RUN; then
        log "[DRY-RUN] Would update Homebrew tap: $HOMEBREW_TAP_REPO"
        return 0
    fi

    # Calculate checksums for each archive
    local linux_x86_sha="" linux_arm_sha="" macos_x86_sha="" macos_arm_sha=""

    cd "$staging_dir"
    if [[ -f "rjest-linux-x86_64.tar.gz" ]]; then
        linux_x86_sha=$(shasum -a 256 rjest-linux-x86_64.tar.gz 2>/dev/null | cut -d' ' -f1 || sha256sum rjest-linux-x86_64.tar.gz | cut -d' ' -f1)
    fi
    if [[ -f "rjest-linux-aarch64.tar.gz" ]]; then
        linux_arm_sha=$(shasum -a 256 rjest-linux-aarch64.tar.gz 2>/dev/null | cut -d' ' -f1 || sha256sum rjest-linux-aarch64.tar.gz | cut -d' ' -f1)
    fi
    if [[ -f "rjest-macos-x86_64.tar.gz" ]]; then
        macos_x86_sha=$(shasum -a 256 rjest-macos-x86_64.tar.gz 2>/dev/null | cut -d' ' -f1 || sha256sum rjest-macos-x86_64.tar.gz | cut -d' ' -f1)
    fi
    if [[ -f "rjest-macos-arm64.tar.gz" ]]; then
        macos_arm_sha=$(shasum -a 256 rjest-macos-arm64.tar.gz 2>/dev/null | cut -d' ' -f1 || sha256sum rjest-macos-arm64.tar.gz | cut -d' ' -f1)
    fi
    cd - >/dev/null

    # Clone or update the tap repo
    local tap_dir=$(mktemp -d)
    git clone "https://github.com/$HOMEBREW_TAP_REPO.git" "$tap_dir"

    # Create the formula
    cat > "$tap_dir/Formula/rjest.rb" << EOF
class Rjest < Formula
  desc "A fast Jest-compatible test runner written in Rust"
  homepage "https://github.com/$GITHUB_REPO"
  version "$version"
  license "MIT"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/$GITHUB_REPO/releases/download/v$version/rjest-macos-x86_64.tar.gz"
      sha256 "$macos_x86_sha"
    else
      url "https://github.com/$GITHUB_REPO/releases/download/v$version/rjest-macos-arm64.tar.gz"
      sha256 "$macos_arm_sha"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/$GITHUB_REPO/releases/download/v$version/rjest-linux-x86_64.tar.gz"
      sha256 "$linux_x86_sha"
    else
      url "https://github.com/$GITHUB_REPO/releases/download/v$version/rjest-linux-aarch64.tar.gz"
      sha256 "$linux_arm_sha"
    end
  end

  def install
    bin.install "jest"
    bin.install "jestd"
  end

  test do
    system "#{bin}/jest", "--version"
  end
end
EOF

    # Commit and push
    cd "$tap_dir"
    git config user.name "Release Bot"
    git config user.email "release@neul.com"
    git add Formula/rjest.rb
    git commit -m "Update rjest to v$version" || warn "No changes to commit"
    git push

    cd - >/dev/null
    rm -rf "$tap_dir"

    success "Updated Homebrew tap"
}

# Cleanup function
cleanup() {
    if [[ -n "${STAGING_DIR:-}" && -d "$STAGING_DIR" ]]; then
        rm -rf "$STAGING_DIR"
    fi
}

# Main function
main() {
    parse_args "$@"

    cd "$REPO_ROOT"

    if $DRY_RUN; then
        log "=== DRY RUN MODE ==="
    fi

    check_dependencies

    local version
    version=$(get_version)
    log "Version: $version"

    # Determine which targets to build
    local targets=()
    if [[ -n "$SPECIFIC_TARGET" ]]; then
        if [[ -z "${TARGET_TRIPLES[$SPECIFIC_TARGET]:-}" ]]; then
            die "Unknown target: $SPECIFIC_TARGET. Valid targets: ${!TARGET_TRIPLES[*]}"
        fi
        targets+=("$SPECIFIC_TARGET")
    elif $BUILD_ALL; then
        # Build all targets
        for target in "${!TARGET_TRIPLES[@]}"; do
            targets+=("$target")
        done
        log "Building all targets (requires 'cross' for non-native targets)"
    else
        # Default: build for current platform only
        local current
        current=$(detect_platform)
        # Normalize for target lookup
        current="${current/arm64/aarch64}"
        if [[ -n "${TARGET_TRIPLES[$current]:-}" ]]; then
            targets+=("$current")
        else
            die "Current platform $current not in target list"
        fi
    fi

    log "Targets: ${targets[*]}"

    # Create staging directory
    STAGING_DIR=$(mktemp -d)
    trap cleanup EXIT

    # Build phase
    if ! $SKIP_BUILD; then
        for target in "${targets[@]}"; do
            build_target "$target" || warn "Failed to build $target"
        done
    else
        log "Skipping build phase (--skip-build)"
    fi

    # Package phase
    local built_any=false
    for target in "${targets[@]}"; do
        if create_archive "$target" "$version" "$STAGING_DIR"; then
            built_any=true
        fi
    done

    if ! $built_any && ! $DRY_RUN; then
        die "No archives were created"
    fi

    # Generate checksums
    generate_checksums "$STAGING_DIR"

    # Create tag and GitHub release
    if ! $SKIP_GITHUB; then
        create_tag "$version"
        create_github_release "$version" "$STAGING_DIR"
    else
        log "Skipping GitHub release (--skip-github)"
    fi

    # Publish to registries
    if $PUBLISH_NPM; then
        publish_npm "$version" "$STAGING_DIR" || warn "Failed to publish to npm"
    fi

    if $PUBLISH_PYPI; then
        publish_pypi "$version" "$STAGING_DIR" || warn "Failed to publish to PyPI"
    fi

    if $PUBLISH_BREW; then
        publish_brew "$version" "$STAGING_DIR" || warn "Failed to update Homebrew tap"
    fi

    success "Release complete!"

    if ! $DRY_RUN; then
        log "Release URL: https://github.com/$GITHUB_REPO/releases/tag/v$version"
        $PUBLISH_NPM && log "npm: https://www.npmjs.com/package/@neul-labs/rjest"
        $PUBLISH_PYPI && log "PyPI: https://pypi.org/project/rjest/"
        $PUBLISH_BREW && log "Homebrew: brew install $HOMEBREW_TAP_REPO/rjest"
    fi
}

main "$@"
