# ZAPITRA

**Zero-copy API Pipeline Integrity, Trace & Resilience Analyzer**

ZAPITRA is an independent Rust reference model and bounded state-space explorer for the proposed BLAKE3-Servil `Pipeline` API. It studies resource ownership, message and room lifecycles, completion events, cancellation, and waits that require worker progress or caller action.

> **Status: design-review tool.** The upstream document dated November 10, 2026 describes a design, not a built runtime. Its skeleton contains `todo!()` bodies. ZAPITRA therefore does not execute BLAKE3 hashing through `Pipeline`, measure zero-copy or zero-allocation behavior, benchmark performance, prove memory safety, or test `io_uring`.

## Why this project exists

The current design gives a single mutable driver access to `Pipeline`. `Message`, `Room`, and `Event` are values. Input may arrive through committed rooms, an owned buffer, a borrowed slice guarded by `Lent`, or a batch. Message records, completion events, and room buffers have bounded capacities.

ZAPITRA provides an external executable model for discussing those contracts. It does not duplicate a runtime that does not exist yet.

## What it checks

- Separate message-record and finished-event capacities.
- Room acquisition before the destination message is known.
- Room assignment only when `commit` occurs.
- Exclusive input paths: rooms, `owned`, `lent`, or `batch`.
- Duplicate caller tags with distinct internal message IDs.
- Completion-order event delivery.
- `Message` drop observation at the next Pipeline call.
- `Room` release when an uncommitted room is dropped.
- Limited `Lent` guard: early drop cancels unfinished work; after all readers finish and the terminal event is materialized, the guard may be released even if `next_event()` has not yet delivered it.
- `Batch<'static>` and digest-storage return as one event.
- Message, room, and `ready()` wait classification.
- Worker-only reachability searches for waits that can make progress without caller action.
- Pipeline-token provenance checks in model operations.
- Invariants checked after every explored transition.

The explorer uses breadth-first search. It reports explored states, transitions, invariant checks, wait classes, and shortest witness paths found within the configured bounds.
The Rust source includes 28 unit tests for the model and explorer.

## Upstream references

- Design: <https://github.com/servil-software-factory/BLAKE3/blob/design/one-stream-api/docs/pipeline-api.md>
- Compile-time skeleton: <https://github.com/servil-software-factory/BLAKE3/tree/design/one-stream-api/docs/experiments/pipeline-api-skeleton>
- Design discussion: <https://github.com/servil-software-factory/BLAKE3/issues/356>
- Implementation planning: <https://github.com/servil-software-factory/BLAKE3/issues/357>

## Build and run

Requirements: Rust stable 1.85 or later, Cargo, rustfmt, Clippy, Git, Bash, Python 3, `zip`, and `sha256sum`. The project has no third-party Cargo dependencies. The scripts use the tools already installed on the developer's Linux system; they do not install packages or change network settings.

```bash
chmod +x scripts/*.sh
./scripts/build.sh
./scripts/verify.sh
```

The binary is `target/release/zapitra`.

```bash
./target/release/zapitra doctor
./target/release/zapitra selfcheck --report reports/selfcheck.json
./target/release/zapitra explore --depth 9 --states 20000 --report reports/explore.json
./scripts/check-upstream-skeleton.sh
```

Use `zapitra help` for all options. The default model bounds are two message records, two rooms, depth eight, and 12,000 states. Increase bounds carefully on low-memory systems.

## Interpreting results

`PASS_BOUNDED_MODEL_ORACLES` means the selected finite state space was exhausted without an invariant or wait-oracle disagreement. `PASS_BOUNDED_MODEL_ORACLES_WITH_LIMIT` means the configured depth or state bound truncated exploration. A bounded pass is evidence about this model and these bounds only.

The upstream script clones the current design branch, records its full commit SHA, and runs the skeleton's documentation tests. Those tests check compiler-accepted and compiler-rejected examples. The skeleton's `todo!()` bodies mean they do not exercise hashing at runtime.

## Explicit model assumptions and open questions

1. **Full completion-event pool:** the design states that completed events have a bounded pool but does not spell out the storage object when that pool is full. ZAPITRA retains the completed result in its existing message record until a consumer pull opens an event slot. This is a model assumption for maintainer review.
2. **`Lent` boundary:** ZAPITRA treats event creation as the point after which dropping the guard does not retroactively abort the hash. The wording could mean event creation or event delivery to the caller. The maintainers should confirm the intended boundary.
3. **Lent worker units:** two abstract work units make partial-start cancellation states observable. They are scheduler-model tokens, not claims about BLAKE3's actual internal task count.
4. **Bounded tags and IDs:** caller tags are represented by `u8` values in the finite model. Duplicate values are allowed. Internal message IDs are separate.
5. **Positive capacities:** the finite model requires `limits.messages >= 1` and `limits.rooms >= 1`. Zero-capacity behavior is outside this release.
6. **Out of scope:** `length_hint` sizing policy, Pipeline destruction with in-flight work, actual worker scheduling, kernel completion queues, and shared budgets across multiple Pipeline instances.

These assumptions are called out so maintainers can correct the model. They are not reported as defects in BLAKE3.

## What ZAPITRA does not claim

- No runtime adapter to a working BLAKE3 `Pipeline` implementation.
- No digest known-answer test against the proposed runtime.
- No allocation counter, zero-copy measurement, throughput benchmark, or latency benchmark.
- No proof of memory safety, data-race freedom, or soundness of the upstream `unsafe fn lent` contract.
- No `io_uring` implementation or fixed-buffer registration.
- No replacement for upstream tests, Shuttle, Miri, mutation testing, or maintainer review.

## Project layout

```text
src/model.rs       Reference state model and invariants
src/explorer.rs    Bounded BFS and worker-only wait oracles
src/main.rs        CLI and fixed self-checks
tests/             Integration-test location for future additions
docs/              Contract matrix, assumptions, review, and publication notes
scripts/           Build, verification, upstream probe, and publishing helpers
.github/workflows/ CI workflow
```

## License

MIT. ZAPITRA is an independent community tool. It is not affiliated with or endorsed by the BLAKE3 maintainers or the Zcash Community Grants program.
## Upstream API consumer probe

Run `./scripts/check-upstream-client.sh` to compile a small external Rust consumer against the pinned BLAKE3 Pipeline API skeleton. This is compile-only: the skeleton methods are `todo!()`, so it does not execute hashing or validate runtime behavior.
