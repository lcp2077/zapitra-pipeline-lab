#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
OWNER="lcp2077"
REPO="zapitra-pipeline-lab"
FULL="$OWNER/$REPO"
DESCRIPTION="ZAPITRA — independent Rust state-space model and contract explorer for the BLAKE3 Pipeline API design"
cd "$ROOT"

for tool in gh git python3; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is required" >&2; exit 2; }
done
gh auth status
[[ -x target/debug/zapitra ]] || { echo "Run ./scripts/build.sh and ./scripts/verify.sh first." >&2; exit 3; }
[[ -s reports/selfcheck.json && -s reports/explore.json ]] || { echo "Verification reports are missing." >&2; exit 4; }
[[ -s reports/upstream-skeleton-revision.txt && -s reports/upstream-doctest.log && -s reports/upstream-client-probe.log ]] || { echo "Run ./scripts/check-upstream-client.sh first." >&2; exit 5; }

python3 - <<'PY'
import json
from pathlib import Path
selfcheck = json.loads(Path("reports/selfcheck.json").read_text())
explore = json.loads(Path("reports/explore.json").read_text())
if selfcheck["fixed_checks_passed"] != selfcheck["fixed_checks_total"]:
    raise SystemExit("Fixed self-checks are incomplete.")
if selfcheck["exploration"]["state_limit_hit"]:
    raise SystemExit("Self-check exploration hit its state budget.")
if explore["state_limit_hit"]:
    raise SystemExit("Extended exploration hit its state budget.")
revision = Path("reports/upstream-skeleton-revision.txt").read_text().strip()
if len(revision) != 40 or any(char not in "0123456789abcdef" for char in revision.lower()):
    raise SystemExit("Upstream revision file does not contain a full commit SHA.")
log = Path("reports/upstream-doctest.log").read_text(errors="replace")
if "test result: ok" not in log:
    raise SystemExit("Upstream doctest log has no successful test result.")
probe = Path("reports/upstream-client-probe.log").read_text(errors="replace")
if "PASS: compile-only client probe" not in probe:
    raise SystemExit("Upstream client compile probe has no PASS marker.")
print("Local model reports: PASS")
print(f"Upstream design revision: {revision}")
PY

if gh repo view "$FULL" >/dev/null 2>&1; then
  echo "STOP: https://github.com/$FULL already exists. No overwrite will be attempted." >&2
  exit 6
fi
if [[ -e .git ]]; then
  echo "STOP: this folder already has .git. Inspect its remote before proceeding." >&2
  git remote -v || true
  exit 7
fi
if [[ -z "$(git config user.name || true)" || -z "$(git config user.email || true)" ]]; then
  echo "Set git config user.name and user.email before publishing." >&2
  exit 8
fi

echo "Repository: https://github.com/$FULL"
echo "Visibility: PUBLIC"
read -r -p 'Type CREATE PUBLIC REPOSITORY to continue: ' CONFIRM
[[ "$CONFIRM" == "CREATE PUBLIC REPOSITORY" ]] || { echo "Cancelled."; exit 0; }

git init -b main
git add .
echo
git status --short
read -r -p 'Type PUBLISH to commit and push these files: ' PUSH_CONFIRM
if [[ "$PUSH_CONFIRM" != "PUBLISH" ]]; then
  git reset --quiet
  rm -rf -- .git
  echo "Cancelled. Source files are unchanged; temporary Git metadata was removed."
  exit 0
fi
git commit -m "ZAPITRA 0.1: Rust Pipeline API contract review"
gh repo create "$FULL" --public --source . --remote origin --push --description "$DESCRIPTION"
echo "Published: https://github.com/$FULL"
echo "Wait for GitHub Actions to pass before posting the upstream comment."
