# Tasks

- [ ] 1.1 `actions/version-pr` has the `auto-merge` input, the required-check guard, and the token guard; proof: `actionlint` and the policy task pass in `mise run verify` on the build host
- [ ] 1.2 This repository's Version PR workflow enables auto-merge with the same guard; proof: the next Version PR shows auto-merge enabled
- [ ] 1.3 README, the example workflow, and the operator instructions describe the input; proof: `cargo test operator_docs` passes on the build host
- [ ] 2.1 The six ctl repositories require their CI check on main and set `auto-merge: true`; proof: each repository's next Version PR merges without a manual merge
