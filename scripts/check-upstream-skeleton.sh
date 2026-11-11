#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/zapitra-upstream.XXXXXX")"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT
REPO="https://github.com/servil-software-factory/BLAKE3.git"
BRANCH="design/one-stream-api"
DEST="$TMP/BLAKE3"

command -v git >/dev/null 2>&1 || { echo "git is required" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "cargo is required" >&2; exit 1; }
mkdir -p "$ROOT/reports"

echo "===== FETCH UPSTREAM DESIGN BRANCH ====="
git clone --quiet --depth 1 --branch "$BRANCH" "$REPO" "$DEST"
REV="$(git -C "$DEST" rev-parse HEAD)"
printf '%s\n' "$REV" | tee "$ROOT/reports/upstream-skeleton-revision.txt"
printf 'Branch: %s\nCommit: %s\n' "$BRANCH" "$REV"

MANIFEST="$DEST/docs/experiments/pipeline-api-skeleton/Cargo.toml"
[[ -f "$MANIFEST" ]] || { echo "Upstream skeleton manifest not found" >&2; exit 1; }
echo "===== RUN UPSTREAM COMPILE-TIME DOCUMENTATION PROBES ====="
set +e
CARGO_TARGET_DIR="$TMP/target" cargo test -j 1 --manifest-path "$MANIFEST" --doc 2>&1 | tee "$ROOT/reports/upstream-doctest.log"
STATUS=${PIPESTATUS[0]}
set -e
if [[ "$STATUS" -ne 0 ]]; then
  echo "FAIL: upstream compile-time probes failed at $REV" >&2
  exit "$STATUS"
fi

echo "PASS: upstream skeleton doctests at $REV"
echo "Scope: compiler/type examples only. The skeleton has todo!() bodies."
