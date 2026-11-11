# Upstream status snapshot

Checked against the design branch `design/one-stream-api` on November 10, 2026.

- Issue #356 defines the single `Pipeline` API design.
- The public design document says the API is not yet built.
- The compile-time skeleton provides signatures with `todo!()` bodies.
- The upstream issue records an earlier external Rust review and says its three findings were incorporated into the design.
- Issue #357 tracks implementation planning.
- The local check supplied with this release passed six compile and six `compile_fail` doctests on Rust 1.99.1 at commit `ca9310fd9d803291706d1f56609b23ad8c8a6a9e`. This confirms compiler examples only.

References:

- https://github.com/servil-software-factory/BLAKE3/issues/356
- https://github.com/servil-software-factory/BLAKE3/issues/357
- https://github.com/servil-software-factory/BLAKE3/blob/design/one-stream-api/docs/pipeline-api.md
- https://github.com/servil-software-factory/BLAKE3/tree/design/one-stream-api/docs/experiments/pipeline-api-skeleton

Always run `scripts/check-upstream-skeleton.sh` before sharing results. The branch can change after this snapshot.
