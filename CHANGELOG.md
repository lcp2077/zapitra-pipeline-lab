# Changelog

## ZAPITRA 0.1 (Cargo package version 0.1.0)

Initial public technical release: an independent Rust reference model and
bounded state-space laboratory for the BLAKE3-Servil Pipeline API design.

### Included

- Message, room, commit, end, abort and drop lifecycle modelling.
- Separate bounded message-record and finished-event pools.
- Worker-only reachability oracles and caller/consumer wait classification.
- `owned`, `lent`, room-based and `Batch` input-path modelling.
- Pipeline identity, duplicate tags, event ordering and invariant checks.
- Fixed contract scenarios, deterministic replay and bounded exploration.
- A compile-only Rust consumer probe against the upstream API skeleton at
  `ca9310fd9d803291706d1f56609b23ad8c8a6a9e`.

### Validation recorded

- 28 Rust unit tests passed.
- 12/12 fixed contract checks passed.
- Extended exploration: 12,652 states, 37,874 transitions and 50,526
  invariant checks. The search reached depth 9 (configured depth limit),
  without reaching the 20,000-state budget. This is bounded evidence,
  not exhaustive verification.
- Deterministic replay produced identical report hashes.
- Upstream API skeleton: 6 compile-pass and 6 compile-fail doctests passed
  at the pinned commit, plus the ZAPITRA compile-only consumer probe.

### Scope and limitations

The upstream skeleton has `todo!()` method bodies. The consumer probe checks
Rust type/signature compatibility only; it is not executed. ZAPITRA does not
validate runtime hashing, performance, zero-allocation behaviour, `io_uring`
registration, or memory safety of an upstream implementation.

The limited Lent guard boundary used by ZAPITRA is a model interpretation:
it permits guard release after the terminal event is materialized, even if
delivery is deferred. This interpretation requires maintainer confirmation.
