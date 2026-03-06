# CMake toolchain file for aarch64-apple-darwin cross-compilation with osxcross

set(CMAKE_SYSTEM_NAME Darwin)
set(CMAKE_SYSTEM_PROCESSOR arm64)

# osxcross location
set(OSXCROSS_TARGET_DIR /usr/local/osxcross/target)
set(OSXCROSS_SDK ${OSXCROSS_TARGET_DIR}/SDK/MacOSX13.3.sdk)

# Use the short wrapper names which properly set up linker paths
set(CMAKE_C_COMPILER ${OSXCROSS_TARGET_DIR}/bin/oa64-clang)
set(CMAKE_CXX_COMPILER ${OSXCROSS_TARGET_DIR}/bin/oa64-clang++)
set(CMAKE_AR ${OSXCROSS_TARGET_DIR}/bin/aarch64-apple-darwin22.4-ar)
set(CMAKE_RANLIB ${OSXCROSS_TARGET_DIR}/bin/aarch64-apple-darwin22.4-ranlib)

# Skip compiler tests that require running binaries (we're cross-compiling)
set(CMAKE_C_COMPILER_WORKS 1)
set(CMAKE_CXX_COMPILER_WORKS 1)

# Find programs
set(CMAKE_FIND_ROOT_PATH ${OSXCROSS_SDK})
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)

# macOS deployment target
set(CMAKE_OSX_DEPLOYMENT_TARGET "11.0")
set(CMAKE_OSX_ARCHITECTURES "arm64")
set(CMAKE_OSX_SYSROOT ${OSXCROSS_SDK})
