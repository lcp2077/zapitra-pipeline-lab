#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

printf '===== ZAPITRA 0.1 / BUILD =====\n'
for tool in rustc cargo rustfmt; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is missing. Use the Rust toolchain already installed on this system."
done
cargo clippy --version >/dev/null 2>&1 || fail "Clippy is missing from the active Rust toolchain."

RUST_VERSION="$(rustc --version | awk '{print $2}')"
if [[ "$(printf '%s\n' 1.85 "$RUST_VERSION" | sort -V | head -n 1)" != "1.85" ]]; then
  fail "Rust 1.85 or later is required; found $RUST_VERSION"
fi

rustc --version
cargo --version
cargo clippy --version

printf '\n===== FORMAT RUST SOURCE =====\n'
cargo fmt --all
printf '\n===== BUILD RELEASE / ONE JOB =====\n'
cargo build --locked --release -j 1
printf '\nBuild complete: %s/target/release/zapitra\n' "$ROOT"
printf 'Next: ./scripts/verify.sh\n'
