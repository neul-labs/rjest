#!/usr/bin/env bash
#
# release.sh - Build and publish rjest releases to GitHub
#
# Usage:
#   ./scripts/release.sh [OPTIONS]
#
# Options:
#   --dry-run       Show what would be done without executing
#   --skip-build    Use existing binaries (skip cargo build)
#   --target=TARGET Build only for specific target (e.g., linux-x86_64)
#   --draft         Create release as draft
#   --help          Show this help message
#
# Environment:
#   GITHUB_TOKEN    GitHub token for gh CLI (optional if already authenticated)
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
SPECIFIC_TARGET=""

# All supported targets
declare -A TARGET_TRIPLES=(
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-aarch64"]="aarch64-unknown-linux-gnu"
    ["macos-x86_64"]="x86_64-apple-darwin"
    ["macos-arm64"]="aarch64-apple-darwin"
    ["windows-x86_64"]="x86_64-pc-windows-msvc"
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
    sed -n '3,17p' "$0" | sed 's/^#//' | sed 's/^ //'
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
            --target=*)
                SPECIFIC_TARGET="${1#*=}"
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
    command -v gh >/dev/null 2>&1 || missing+=("gh")
    command -v tar >/dev/null 2>&1 || missing+=("tar")

    if [[ ${#missing[@]} -gt 0 ]]; then
        die "Missing dependencies: ${missing[*]}"
    fi

    # Check gh authentication
    if ! gh auth status >/dev/null 2>&1; then
        die "GitHub CLI not authenticated. Run 'gh auth login' first."
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
create_release() {
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

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/neul-labs/rjest/main/install.sh | sh
\`\`\`

Or download the appropriate archive for your platform below.

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

    # Create tag and release
    create_tag "$version"
    create_release "$version" "$STAGING_DIR"

    success "Release complete!"

    if ! $DRY_RUN; then
        log "Release URL: https://github.com/neul-labs/rjest/releases/tag/v$version"
    fi
}

main "$@"
