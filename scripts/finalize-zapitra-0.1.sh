#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
ARCHIVE="$HOME/Downloads/ZAPITRA-0.1.zip"

cd "$ROOT"
for tool in cargo git zip unzip sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is required" >&2; exit 2; }
done

grep -q '^version = "0.1.0"$' Cargo.toml || {
  echo "Cargo package version is not 0.1.0." >&2
  exit 3
}

echo "===== ZAPITRA 0.1 / RELEASE BUILD ====="
./scripts/build.sh

echo
echo "===== FULL LOCAL VERIFICATION ====="
./scripts/verify.sh

echo
echo "===== PINNED UPSTREAM DOCTESTS + EXTERNAL CLIENT PROBE ====="
./scripts/check-upstream-client.sh

echo
echo "===== PACKAGE AND CHECKSUM ====="
./scripts/package.sh

[[ -s "$ARCHIVE" && -s "$ARCHIVE.sha256" ]] || {
  echo "Final archive or checksum is missing." >&2
  exit 4
}
(
  cd "$HOME/Downloads"
  sha256sum -c ZAPITRA-0.1.zip.sha256
  unzip -tq ZAPITRA-0.1.zip
)

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
unzip -q "$ARCHIVE" -d "$TMP"
EXTRACTED="$TMP/zapitra-pipeline-lab"
[[ -f "$EXTRACTED/Cargo.toml" && -f "$EXTRACTED/src/lib.rs" ]] || {
  echo "Archive does not contain the expected Rust source tree." >&2
  exit 5
}
grep -q '^version = "0.1.0"$' "$EXTRACTED/Cargo.toml" || {
  echo "Archive Cargo version does not match 0.1.0." >&2
  exit 6
}
[[ -s "$EXTRACTED/reports/selfcheck.json" && -s "$EXTRACTED/reports/explore.json" ]] || {
  echo "Verification JSON reports are missing from the archive." >&2
  exit 7
}
[[ -s "$EXTRACTED/reports/upstream-client-probe.log" ]] || {
  echo "Upstream consumer probe report is missing from the archive." >&2
  exit 8
}

echo
echo "===== ZAPITRA 0.1 PACKAGE VERIFIED ====="
ls -lh "$ARCHIVE" "$ARCHIVE.sha256"
cat "$ARCHIVE.sha256"
echo "Package: $ARCHIVE"
echo "Next: inspect docs/UPSTREAM-COMMENT.md, then publish GitHub and post to BLAKE3 issue #356."
