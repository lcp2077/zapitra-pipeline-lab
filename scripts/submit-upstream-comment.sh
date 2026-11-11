#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v gh >/dev/null 2>&1 || { echo "GitHub CLI is required" >&2; exit 2; }
gh auth status

if ! gh repo view lcp2077/zapitra-pipeline-lab >/dev/null 2>&1; then
  echo "Publish https://github.com/lcp2077/zapitra-pipeline-lab first." >&2
  exit 3
fi
[[ -s reports/upstream-skeleton-revision.txt ]] || { echo "Run check-upstream-skeleton.sh first." >&2; exit 4; }
[[ -s reports/explore.json ]] || { echo "Run verify.sh first." >&2; exit 5; }

RUNS="$(gh run list --repo lcp2077/zapitra-pipeline-lab --branch main --limit 1 --json status,conclusion)"
python3 - "$RUNS" <<'PY'
import json, sys
runs = json.loads(sys.argv[1])
if not runs or runs[0].get("status") != "completed" or runs[0].get("conclusion") != "success":
    raise SystemExit(f"Latest main CI is not green: {runs[:1]}")
print("Latest GitHub Actions run: completed / success")
PY

REVISION="$(tr -d '\r\n' < reports/upstream-skeleton-revision.txt)"
[[ "$REVISION" =~ ^[0-9a-fA-F]{40}$ ]] || { echo "Invalid upstream commit SHA." >&2; exit 6; }
BODY="$(mktemp "${TMPDIR:-/tmp}/zapitra-comment.XXXXXX")"
trap 'rm -f -- "$BODY"' EXIT
cat docs/UPSTREAM-COMMENT.md > "$BODY"
printf '\nUpstream design revision checked: `%s`.\n' "$REVISION" >> "$BODY"

python3 - "$BODY" <<'PY'
import pathlib, sys
text = pathlib.Path(sys.argv[1]).read_text()
words = len(text.split())
print(f"Prepared upstream comment: {words} words")
if words > 150:
    raise SystemExit("Comment exceeds the maintainers' 150-word comment budget.")
PY

cat "$BODY"
echo
read -r -p 'Type POST COMMENT to publish this text to BLAKE3 issue #356: ' CONFIRM
[[ "$CONFIRM" == "POST COMMENT" ]] || { echo "Cancelled."; exit 0; }
gh issue comment 356 --repo servil-software-factory/BLAKE3 --body-file "$BODY"
echo "Comment posted to https://github.com/servil-software-factory/BLAKE3/issues/356"
