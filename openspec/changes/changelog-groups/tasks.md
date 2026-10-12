# Tasks

- [x] 1.1 Parse `verctl:group` and reject unknown values; proof: fragment tests on the build host
  - 2026-10-12 UTC: `a_group_key_names_the_heading_and_no_package`, `a_fragment_without_a_group_key_is_ungrouped`, and `an_unknown_group_fails` passed in CI `verify` on [#123][verctl#123].
- [x] 1.2 Render grouped sections, flat when no fragment names a group, ungrouped entries under Changed; proof: release tests on the build host
  - 2026-10-12 UTC: `a_section_with_no_group_stays_flat` and `an_ungrouped_fragment_lands_in_the_default_group` passed in CI `verify` on [#123][verctl#123].
    Every existing changelog test passed unchanged.
- [x] 1.3 Operator docs name the key; proof: `cargo test operator_docs` on the build host
  - 2026-10-12 UTC: `operator_docs` passed in the same `verify` run, so the committed skill and instructions match their templates.

Not verified: `cargo semver-checks` on this branch, because CI skips it for pull requests into `next`.
Against 0.3.16 it fails as a patch with `constructible_struct_adds_field` on `Fragment.group`, which is why this lands on `next`.

[verctl#123]: https://github.com/victor-software-house/verctl/pull/123
