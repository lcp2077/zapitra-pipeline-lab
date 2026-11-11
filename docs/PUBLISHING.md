# Publishing and contribution checklist — ZAPITRA 0.1

1. Run `./scripts/build.sh` and `./scripts/verify.sh`.
2. Run `./scripts/check-upstream-client.sh`. It pins the upstream design commit,
   executes the upstream compile-time doctests and compiles the ZAPITRA client
   probe against the actual skeleton.
3. Review `reports/` and all drafts under `docs/`.
4. Run `./scripts/package.sh` to create `~/Downloads/ZAPITRA-0.1.zip` and its
   SHA-256 file.
5. Run `./scripts/publish-github.sh` to create the public repository. It asks
   for explicit confirmation and refuses to overwrite an existing repository.
6. Confirm GitHub Actions passes for the pushed `main` commit.
7. Run `./scripts/publish-release.sh` to publish tag `v0.1.0` and the ZIP.
8. Review `docs/UPSTREAM-COMMENT.md`; post it to BLAKE3 issue #356 using the
   prepared submission script, after checking that the pinned upstream commit
   and findings still match.
9. Reply to Zooko's public call using `docs/X-REPLY.md`, adding the published
   repository URL.

The upstream skeleton is a compile-time design artifact with `todo!()` bodies.
Neither its doctests nor the compile-only consumer probe exercise runtime
hashing, performance, zero allocation, `io_uring`, or memory safety.
