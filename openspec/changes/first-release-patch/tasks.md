# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Refuse

- [ ] 1.1 `bump::apply` refuses `minor` and `major` at exactly `0.0.0` unless `first_minor` is set; proof: `cargo test --test bump` passes the patch, minor, major, override, and none cases
- [ ] 1.2 `PackageSpec` reads `first_minor`, false when unsaid; proof: `prepare_reads_first_minor_from_the_package` loads it from `.ctl/ver.yaml`
- [ ] 1.3 `check` plans the pending fragments; proof: `check_refuses_what_prepare_refuses` passes
- [ ] 1.4 `verify` runs `check` through a `fragments` task; proof: `mise run verify` passes

## 2. Document and ship

- [ ] 2.1 The instructions template, the operator skill template, the README, and `examples/ver.yaml` name the rule and `first_minor`; proof: `cargo test operator_docs` passes after regeneration
- [ ] 2.2 A `patch` changeset ships it; proof: `verctl status` lists it
- [ ] 2.3 Close VER-042 when the pull request merges; proof: `mise run q check` passes
