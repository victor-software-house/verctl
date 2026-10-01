# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Refuse

- [x] 1.1 `bump::apply` refuses `minor` and `major` at exactly `0.0.0` unless `first_minor` is set; proof: `cargo test --test bump` passes the patch, minor, major, override, and none cases
  - 2026-10-01 15:40 UTC: `cargo test --all-features --test bump` passed 26 tests on the build host at [ca6de8f], including `a_new_package_takes_a_patch`, `a_new_package_refuses_minor_and_major`, `a_pre_release_of_0_0_0_is_still_new`, `first_minor_opens_a_new_package_at_0_1_0`, `none_leaves_a_new_package_alone`, and `the_first_release_rule_ends_at_0_0_1`. The guard compares major, minor, and patch, so a `0.0.0` pre-release counts as new, after review on [PR 101][verctl#101].
- [x] 1.2 `PackageSpec` reads `first_minor`, false when unsaid; proof: `prepare_reads_first_minor_from_the_package` loads it from `.ctl/ver.yaml`
  - 2026-10-01 15:40 UTC: passed in the same run at [ca6de8f].
- [x] 1.3 `check` plans the pending fragments; proof: `check_refuses_what_prepare_refuses` passes
  - 2026-10-01 15:40 UTC: `cargo test --all-features --test status` passed 15 tests at [ca6de8f], including `check_refuses_what_prepare_refuses` and `check_ok_when_dir_missing`. With no fragments, `check` returns before reading `.ctl/ver.yaml`.
- [x] 1.4 `verify` runs `check` through a `fragments` task; proof: `mise run verify` passes
  - 2026-10-01 15:40 UTC: `mise run verify` passed on the build host at [ca6de8f]; the `fragments` task printed `ok  1 fragment(s)`. CI [run 36884790598] passed `Plan` and `verify`.

## 2. Document and ship

- [x] 2.1 The instructions template, the operator skill template, the README, and `examples/ver.yaml` name the rule and `first_minor`; proof: `cargo test operator_docs` passes after regeneration
  - 2026-10-01 15:40 UTC: `src/instructions.md` and `skills/verctl/SKILL.md` regenerated with `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs`; `operator_docs` passed inside `mise run verify` at [ca6de8f].
- [x] 2.2 A `patch` changeset ships it; proof: `verctl status` lists it
  - 2026-10-01 15:40 UTC: `.changeset/first-release-check.md` declares `verctl: patch`; the `fragments` task counted it.
- [x] 2.3 Close VER-042 when the pull request merges; proof: `mise run q check` passes
  - 2026-10-01 15:40 UTC: VER-042 archived with `mise run q archive` on [PR 101][verctl#101], which closes it on merge; `mise run q check` passed.

Not verified: a real Version PR refusing a `minor` fragment on a `0.0.0` package in CI. The refusal is proven by the tests above, not by a live `prepare --pr` run.

[ca6de8f]: https://github.com/victor-software-house/verctl/pull/101/commits/ca6de8f
[verctl#101]: https://github.com/victor-software-house/verctl/pull/101
[run 36884790598]: https://github.com/victor-software-house/verctl/actions/runs/36884790598
