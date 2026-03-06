# macOS Cross-Compilation with Docker

This directory contains Docker configurations for cross-compiling rjest to macOS from Linux.

## Quick Start (Recommended)

The easiest approach uses pre-built images from `joseluisq/rust-linux-darwin-builder`:

```bash
# Build the Docker images
./scripts/build-macos-images.sh

# Cross-compile (requires `cross` installed: cargo install cross)
cross build --release --target x86_64-apple-darwin
cross build --release --target aarch64-apple-darwin
```

## Files

- `Dockerfile.macos-x86_64` - Image for x86_64-apple-darwin (Intel Mac)
- `Dockerfile.macos-aarch64` - Image for aarch64-apple-darwin (Apple Silicon)
- `Dockerfile.osxcross` - Custom osxcross build (requires your own SDK)

## Using Pre-built Images Directly

If you don't want to build the images, you can use the base image directly. Edit `Cross.toml`:

```toml
[target.x86_64-apple-darwin]
image = "joseluisq/rust-linux-darwin-builder:1.85"

[target.aarch64-apple-darwin]
image = "joseluisq/rust-linux-darwin-builder:1.85"
```

Then set the appropriate environment variables:

```bash
# For x86_64
cross build --release --target x86_64-apple-darwin

# For aarch64
cross build --release --target aarch64-apple-darwin
```

## Building Your Own osxcross Image

If you need a specific SDK version or more control, use `Dockerfile.osxcross`:

1. **Obtain the macOS SDK** (requires a Mac):
   ```bash
   # On a Mac with Xcode installed
   git clone https://github.com/tpoechtrager/osxcross
   cd osxcross
   ./tools/gen_sdk_package.sh
   # This creates MacOSX<version>.sdk.tar.xz
   ```

2. **Place SDK in docker/sdk/**:
   ```bash
   mkdir -p docker/sdk
   cp MacOSX14.0.sdk.tar.xz docker/sdk/
   ```

3. **Build the image**:
   ```bash
   docker build -f docker/Dockerfile.osxcross \
     --build-arg SDK_VERSION=14.0 \
     -t rjest-osxcross:latest .
   ```

4. **Update Cross.toml** to use your custom image:
   ```toml
   [target.x86_64-apple-darwin]
   image = "rjest-osxcross:latest"
   ```

## Licensing Notice

Cross-compiling to macOS requires the Apple SDK. The SDK is proprietary and subject to Apple's licensing terms. Options:

1. **Use on a Mac** - Native compilation avoids these issues
2. **GitHub Actions** - Uses official macOS runners (already configured in CI)
3. **Verify compliance** - If using osxcross, ensure you have rights to the SDK

The pre-built `joseluisq/rust-linux-darwin-builder` images include SDKs - verify their licensing is acceptable for your use case.

## Troubleshooting

### "SDK not found" errors
Ensure the SDK tarball is properly named and placed in `docker/sdk/`.

### Linker errors
Some crates require additional configuration. Add to `.cargo/config.toml`:
```toml
[target.x86_64-apple-darwin]
linker = "o64-clang"

[target.aarch64-apple-darwin]
linker = "oa64-clang"
```

### Missing system libraries
Some crates need macOS system libraries. The Dockerfiles include common ones, but you may need to add more to handle specific dependencies.
