#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
UPSTREAM_URL="https://github.com/servil-software-factory/BLAKE3.git"
UPSTREAM_COMMIT="ca9310fd9d803291706d1f56609b23ad8c8a6a9e"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$ROOT/reports"
rm -f "$ROOT/reports/upstream-skeleton-revision.txt"       "$ROOT/reports/upstream-doctest.log"       "$ROOT/reports/upstream-client-probe.log"

for tool in git cargo rustfmt; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is required" >&2; exit 2; }
done

echo "===== FORMAT EXTERNAL CLIENT PROBE ====="
rustfmt --edition 2024 --check "$ROOT/probes/upstream-client-probe.rs"

echo "===== FETCH PINNED BLAKE3 DESIGN COMMIT ====="
git clone --quiet --filter=blob:none --no-checkout "$UPSTREAM_URL" "$TMP/BLAKE3"
git -C "$TMP/BLAKE3" fetch --quiet --depth 1 origin "$UPSTREAM_COMMIT"
git -C "$TMP/BLAKE3" checkout --quiet --detach "$UPSTREAM_COMMIT"
ACTUAL="$(git -C "$TMP/BLAKE3" rev-parse HEAD)"
[[ "$ACTUAL" == "$UPSTREAM_COMMIT" ]] || { echo "Upstream commit mismatch: $ACTUAL" >&2; exit 3; }
printf '%s\n' "$ACTUAL" | tee "$ROOT/reports/upstream-skeleton-revision.txt"

SKELETON="$TMP/BLAKE3/docs/experiments/pipeline-api-skeleton"
[[ -f "$SKELETON/Cargo.toml" && -f "$SKELETON/src/lib.rs" ]] || { echo "Pinned skeleton is missing." >&2; exit 4; }

echo "===== UPSTREAM COMPILE-PASS / COMPILE-FAIL DOCTESTS ====="
if ! cargo test --manifest-path "$SKELETON/Cargo.toml" --doc > "$ROOT/reports/upstream-doctest.log" 2>&1; then
  cat "$ROOT/reports/upstream-doctest.log"
  exit 5
fi
cat "$ROOT/reports/upstream-doctest.log"

# Build an external consumer crate against the real upstream path dependency.
PROBE="$TMP/consumer-probe"
mkdir -p "$PROBE/examples"
cp "$ROOT/probes/upstream-client-probe.rs" "$PROBE/examples/client_probe.rs"
cat > "$PROBE/Cargo.toml" <<EOF
[package]
name = "zapitra-upstream-client-probe"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
pipeline-api-skeleton = { path = "$SKELETON" }
EOF

echo "===== COMPILE ZAPITRA CONSUMER AGAINST UPSTREAM SIGNATURES ====="
if ! cargo check --manifest-path "$PROBE/Cargo.toml" --example client_probe > "$ROOT/reports/upstream-client-probe.log" 2>&1; then
  cat "$ROOT/reports/upstream-client-probe.log"
  exit 6
fi
cat "$ROOT/reports/upstream-client-probe.log"
echo "PASS: compile-only client probe" | tee -a "$ROOT/reports/upstream-client-probe.log"
echo "PASS: pinned upstream doctests and ZAPITRA compile-only client probe"
