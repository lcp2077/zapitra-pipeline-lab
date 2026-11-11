#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="v0.1.0"
ARCHIVE="$HOME/Downloads/ZAPITRA-0.1.zip"
cd "$ROOT"
command -v gh >/dev/null 2>&1 || { echo "gh is required" >&2; exit 2; }
command -v git >/dev/null 2>&1 || { echo "git is required" >&2; exit 2; }
REMOTE="$(git remote get-url origin 2>/dev/null || true)"
[[ "$REMOTE" == *"lcp2077/zapitra-pipeline-lab"* ]] || { echo "Unexpected or missing origin: $REMOTE" >&2; exit 3; }
[[ -z "$(git status --porcelain)" ]] || { echo "Working tree is not clean." >&2; git status --short; exit 4; }
if git rev-parse "$VERSION" >/dev/null 2>&1; then
  echo "Tag $VERSION already exists; refusing to replace it." >&2
  exit 5
fi
RUNS="$(gh run list --repo lcp2077/zapitra-pipeline-lab --branch main --limit 1 --json status,conclusion,headSha)"
HEAD_SHA="$(git rev-parse HEAD)"
python3 - "$RUNS" "$HEAD_SHA" <<'PY_RELEASE'
import json, sys
runs = json.loads(sys.argv[1])
expected = sys.argv[2]
if not runs:
    raise SystemExit("No main CI run was found.")
run = runs[0]
if run.get("headSha") != expected:
    raise SystemExit(f"Latest CI is for {run.get('headSha')}, expected {expected}.")
if run.get("status") != "completed" or run.get("conclusion") != "success":
    raise SystemExit(f"CI for current HEAD is not green: {run}")
print(f"CI for current HEAD {expected}: completed / success")
PY_RELEASE
./scripts/package.sh
[[ -s "$ARCHIVE" && -s "$ARCHIVE.sha256" ]] || { echo "Release ZIP/checksum missing." >&2; exit 6; }
(
  cd "$HOME/Downloads"
  sha256sum -c ZAPITRA-0.1.zip.sha256
)
read -r -p 'Type CREATE RELEASE to publish v0.1.0: ' CONFIRM
[[ "$CONFIRM" == "CREATE RELEASE" ]] || { echo "Cancelled."; exit 0; }
git push origin main
git tag -a "$VERSION" -m "ZAPITRA 0.1"
git push origin "$VERSION"
gh release create "$VERSION" "$ARCHIVE" "$ARCHIVE.sha256" \
  --title "ZAPITRA 0.1 — BLAKE3 Pipeline API Contract Review" \
  --notes-file CHANGELOG.md
echo "Release: https://github.com/lcp2077/zapitra-pipeline-lab/releases/tag/$VERSION"
