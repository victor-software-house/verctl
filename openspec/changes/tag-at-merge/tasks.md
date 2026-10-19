# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Tag order

- [ ] 1.1 `tags_before_upload` puts a crates.io or release-only tag first and a tag with an unchecked upload after; proof: the `tag_order` tests in `src/publish.rs` pass in CI
- [ ] 1.2 `run_pending` creates the early tags before any upload and the late ones after; proof: code review of `run_pending`

## 2. docs.rs

- [ ] 2.1 `released_crates` asks the crates.io index before waiting; proof: code review of `released_crates`

## 3. Docs

- [ ] 3.1 The operator instructions and README describe the order; proof: `cargo test operator_docs` passes in CI
- [ ] 3.2 release-state-machine, release-from-shallow-and-drifted-trees, VER-011, and VER-072 record the operator decision; proof: `openspec validate --all --strict` passes in CI
