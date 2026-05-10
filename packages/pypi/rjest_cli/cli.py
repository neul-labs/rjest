"""CLI wrapper for rjest binary"""
import os
import sys
import platform
import subprocess


def get_binary_path():
    """Get the path to the rjest binary"""
    binary_name = "jest.exe" if platform.system() == "Windows" else "jest"

    # Try to find in common locations
    locations = [
        os.path.join(os.path.dirname(__file__), "bin", binary_name),
        os.path.join(sys.prefix, "bin", binary_name),
        os.path.join(sys.prefix, "Scripts", binary_name),
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
