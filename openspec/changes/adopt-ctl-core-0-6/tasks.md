# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Adopt

- [x] 1.1 Pin ctl-core `=0.6.3`; proof: `Cargo.lock` resolves ctl-core 0.6.3
- [x] 1.2 Move package-name columns to `Table::id_column`, keeping targets and checks as tokens; proof: `rg 'token_column' src/presentation.rs` lists only the target and check tables
- [x] 1.3 Expect borderless records in the publish and presentation tests, and compare JSON as parsed values; proof: `mise run verify` passes
- [ ] 1.4 Close VER-040 when the pull request merges; proof: `mise run q check` passes
