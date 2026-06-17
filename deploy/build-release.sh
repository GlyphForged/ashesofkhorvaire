#!/bin/sh
set -eu

if ! command -v cargo >/dev/null 2>&1; then
  echo "Cargo is not available. Install Rust with rustup, then run this script again." >&2
  exit 1
fi

case "$(uname -m)" in
  aarch64|arm64) echo "Building ARM64 release for $(uname -m)..." ;;
  armv7l|armv8l) echo "Building 32-bit ARM release for $(uname -m)..." ;;
  *) echo "Warning: building for unexpected architecture $(uname -m)." >&2 ;;
esac

cargo test --locked
cargo build --locked --release

echo "Release binary ready at target/release/ashes-wiki"
