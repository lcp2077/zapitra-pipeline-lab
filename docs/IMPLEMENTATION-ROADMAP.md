# Implementation roadmap — ZAPITRA 0.1

ZAPITRA 0.1 is a design-review and test-laboratory release for the BLAKE3
Pipeline API proposal.

## Completed in 0.1

- Independent state model for ownership and message/room lifecycle rules.
- Bounded event/message pools and backpressure classification.
- Worker-only reachability checks for message, room and ready waits.
- `owned`, `lent`, rooms and batch contract scenarios.
- 28 unit tests and 12 fixed self-checks.
- Deterministic replay and bounded state exploration.
- Compile-only consumer probe against upstream skeleton commit `ca9310fd9d803291706d1f56609b23ad8c8a6a9e`.

## Not claimed

The upstream skeleton bodies are `todo!()`. Runtime hashing, measured
zero-allocation, performance, real `io_uring` I/O, runtime concurrency and
memory-safety validation remain outside this release.

## Next review targets

1. Ask maintainers to confirm the Lent guard completion boundary.
2. Ask maintainers to confirm deferred finished-event behavior under a full
   event queue.
3. Re-run the consumer probe when the upstream implementation is available.
4. Add runtime integration tests only against implemented upstream methods.
