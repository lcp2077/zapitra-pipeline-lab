# Questions for the Pipeline maintainers

These are requests to confirm the model. They are not reports of defects.

1. **Completion-event pool full:** the text says finished events have a bounded pool. Should a completed result retain its message record until `next_event` frees an event slot, as ZAPITRA models?
2. **Limited `Lent` boundary:** ZAPITRA releases the guard after all readers finish and the terminal event is materialized. Is that the intended lifetime boundary, even when a full event pool delays delivery through `next_event()`?
3. **Worker scheduling abstraction:** does the implementation expose any progress or cancellation point that a consumer-side test harness should model beyond the documented `ready()` and worker completion behavior?
4. **Zero-byte room commit:** is `commit(..., written = 0)` valid? If so, is an empty fragment committed or should it be treated like an unused room?
5. **Failure reporting:** are worker failures represented by `Aborted` alone, or is another error surface planned?
6. **`io_uring` lifetime:** should a future adapter retain a `Room` until the corresponding completion queue entry (CQE) is observed, including cancellation races?

The model should be changed to match the maintainers' answer. Its current assumptions are recorded in `README.md` and `docs/ARCHITECTURE.md`.
