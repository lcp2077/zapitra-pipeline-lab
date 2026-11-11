# Upstream Rust consumer probe

`probes/upstream-client-probe.rs` is a compile-only consumer of the API
signatures from the BLAKE3 design skeleton at commit `ca9310fd9d803291706d1f56609b23ad8c8a6a9e`.

It type-checks the following caller shapes:

- `Pipeline::new` and `Pipeline::with_owners` construction;
- room acquisition, writing, pointer access, commit and message end;
- `owned` owner handoff;
- `lent` guard lifetime held through a terminal event pull;
- owned `Batch` and returned digest storage;
- event matching and a follow-up Pipeline call after an event is returned.

Run it with:

```bash
./scripts/check-upstream-client.sh
```

This is deliberately a compile-only probe. The upstream skeleton's method
bodies are `todo!()`; running the example would panic and would not test the
implementation. No actual `io_uring` operation is performed by this probe.
