Hello John, Zooko, and BLAKE3 contributors,

I saw the call for Rust developers and AI agents to review the new Pipeline
API. I built ZAPITRA 0.1, an independent Rust contract model and bounded
state-space review laboratory for this design.

The model covers message and room lifecycles, room acquisition before routing,
commit semantics, separate bounded message/event pools, backpressure,
worker-only progress, caller/consumer wait classification, dropped messages,
`owned`, `lent`, and owned `Batch` storage.

Validation in this release:

- 28 Rust unit tests passed.
- 12/12 fixed contract checks passed.
- Bounded exploration visited 12,652 states, checked 37,874 transitions and
  50,526 invariants. It reached the configured depth limit of 9, not the
  20,000-state budget; this is bounded evidence, not an exhaustive proof.
- The upstream skeleton at `ca9310fd9d803291706d1f56609b23ad8c8a6a9e` passed its six compile-pass and six
  compile-fail doctests.
- A separate ZAPITRA Rust consumer probe compiles against the same skeleton.

I am not claiming runtime conformance: the skeleton method bodies are
`todo!()`, and the consumer probe is compile-only. No runtime hashing,
zero-allocation, performance, `io_uring`, or memory-safety claim is made.

Two model assumptions would benefit from maintainer review:

1. When the event queue is full and a message record is retained for a
   materialized completion, the next message wait is classified as requiring
   the consumer to pull an event, even if worker work remains elsewhere.
2. The limited Lent model allows its guard to be released after the terminal
   event is materialized, even if delivery is deferred by event backpressure.
   Please confirm whether materialization, rather than delivery, is the
   intended lifetime boundary.

The project and reproducible checks are available here:
https://github.com/lcp2077/zapitra-pipeline-lab

I would appreciate corrections to the model assumptions and guidance on
additional compile-time or runtime tests that would be useful once the
Pipeline implementation is available.
