#!/usr/bin/env bash
# Build amplifier-ffi for the current platform and pack a NuGet package.
#
# Usage:
#   ./pack.sh                  # build + pack for current platform
#   ./pack.sh --skip-build     # pack only (expects pre-built binaries in runtimes/)
#
# The script must be run from the nuget/ directory (bindings/dotnet/nuget).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
NUGET_DIR="$SCRIPT_DIR"

SKIP_BUILD=false
if [[ "${1:-}" == "--skip-build" ]]; then
  SKIP_BUILD=true
fi

mkdir -p "$NUGET_DIR/runtimes/win-x64/native" \
         "$NUGET_DIR/runtimes/linux-x64/native"

if [[ "$SKIP_BUILD" == false ]]; then
  echo "Building amplifier-ffi (release)..."
  cargo build --release -p amplifier-ffi --manifest-path "$REPO_ROOT/Cargo.toml"

  # Copy built library to the appropriate runtime slot
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      cp "$REPO_ROOT/target/release/amplifier_ffi.dll" \
         "$NUGET_DIR/runtimes/win-x64/native/"
      ;;
    Linux)
      cp "$REPO_ROOT/target/release/libamplifier_ffi.so" \
         "$NUGET_DIR/runtimes/linux-x64/native/"
      ;;
    *)
      echo "Warning: unsupported platform $(uname -s), skipping copy"
      ;;
  esac
fi

echo "Packing NuGet package..."
nuget pack "$NUGET_DIR/Amplifier.FFI.Runtime.nuspec" -OutputDirectory "$NUGET_DIR/out"

echo "Done. Package written to $NUGET_DIR/out/"
