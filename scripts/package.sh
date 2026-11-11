#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
NAME="ZAPITRA-0.1.zip"
OUTDIR="${OUTPUT_DIR:-$HOME/Downloads}"
command -v zip >/dev/null 2>&1 || { echo "zip is required" >&2; exit 2; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required" >&2; exit 2; }
[[ -f "$ROOT/Cargo.lock" ]] || { echo "Cargo.lock is missing" >&2; exit 3; }
grep -q '^version = "0.1.0"$' "$ROOT/Cargo.toml" || { echo "Cargo package is not version 0.1.0" >&2; exit 4; }
mkdir -p "$OUTDIR"
OUTDIR="$(cd "$OUTDIR" && pwd)"
cd "$(dirname "$ROOT")"
rm -f -- "$OUTDIR/$NAME" "$OUTDIR/$NAME.sha256"
zip -qr "$OUTDIR/$NAME" "$(basename "$ROOT")" \
  -x '*/target/*' '*/.git/*' '*/.env' '*/.env.*' \
     '*/src/*.pre-*' '*/src/*.bak' '*/src/*.backup'
(
  cd "$OUTDIR"
  sha256sum "$NAME" > "$NAME.sha256"
  sha256sum -c "$NAME.sha256"
  unzip -tq "$NAME"
)
printf 'Created: %s/%s\n' "$OUTDIR" "$NAME"
printf 'Checksum: %s/%s.sha256\n' "$OUTDIR" "$NAME"
cat "$OUTDIR/$NAME.sha256"
