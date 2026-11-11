# Contributing

Contributions should improve a documented contract model or add a reproducible test.

1. Read `README.md`, `docs/ARCHITECTURE.md`, and `docs/CONTRACT-MATRIX.md`.
2. Keep the reference model separate from any future upstream runtime adapter.
3. Add a unit test for every state transition or invariant change.
4. State whether a behavior comes from the upstream design or is a model assumption.
5. Run `./scripts/build.sh` and `./scripts/verify.sh`.
6. Include toolchain version, upstream commit SHA, limits, and a minimal state trace in reports.

Do not describe a model witness as an upstream defect until it is reproduced against an executable upstream implementation.
