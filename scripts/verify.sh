#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p reports

printf '===== ZAPITRA / FORMAT =====\n'
cargo fmt --all -- --check
printf '\n===== ZAPITRA / CLIPPY =====\n'
cargo clippy --locked --all-targets -j 1 -- -D warnings
printf '\n===== ZAPITRA / UNIT TESTS =====\n'
cargo test --locked -j 1
printf '\n===== ZAPITRA / DEBUG BUILD =====\n'
cargo build --locked -j 1

printf '\n===== ZAPITRA / FIXED CHECKS + BOUNDED EXPLORATION =====\n'
./target/debug/zapitra selfcheck --depth 8 --states 12000 --report reports/selfcheck.json
printf '\n===== ZAPITRA / EXTENDED STATE EXPLORATION =====\n'
./target/debug/zapitra explore --depth 9 --states 20000 --report reports/explore.json

printf '\n===== ZAPITRA / JSON VALIDATION =====\n'
python3 - <<'PY'
import json
from pathlib import Path

selfcheck = json.loads(Path("reports/selfcheck.json").read_text())
explore = json.loads(Path("reports/explore.json").read_text())
assert selfcheck["fixed_checks_passed"] == selfcheck["fixed_checks_total"], selfcheck
assert selfcheck["fixed_checks_total"] == 12, selfcheck
assert not selfcheck["exploration"]["state_limit_hit"], selfcheck["exploration"]
assert not explore["state_limit_hit"], explore
assert explore["states_explored"] > 1
assert explore["invariant_checks"] >= explore["states_explored"]
assert explore["message_waits"]["worker_can_progress"] > 0
assert explore["message_waits"]["caller_must_progress"] > 0
assert explore["message_waits"]["consumer_must_pull"] > 0
assert explore["room_waits"]["worker_can_progress"] > 0
assert explore["room_waits"]["caller_must_progress"] > 0
assert explore["ready_waits"]["worker_can_progress"] > 0
assert explore["ready_waits"]["no_worker_can_produce"] > 0
print("PASS reports/selfcheck.json")
print("PASS reports/explore.json")
print("States:", explore["states_explored"])
print("Transitions:", explore["transitions_checked"])
print("Invariant checks:", explore["invariant_checks"])
print("Bounded verdict:", explore["verdict"])
PY

printf '\n===== ZAPITRA / DETERMINISTIC REPLAY =====\n'
./target/debug/zapitra explore --depth 8 --states 12000 --report reports/replay-a.json >/dev/null
./target/debug/zapitra explore --depth 8 --states 12000 --report reports/replay-b.json >/dev/null
cmp reports/replay-a.json reports/replay-b.json
sha256sum reports/replay-a.json reports/replay-b.json
printf '\nPASS: model checks and deterministic replay completed.\n'
printf 'NOTE: these results concern ZAPITRA model states, not upstream runtime hashing.\n'
