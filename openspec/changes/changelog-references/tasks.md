# Tasks

- [x] 1.1 First-parent commit that added a fragment, and the `(#N)` subject parse; proof: unit tests on a temporary repository, on the build host
- [x] 1.2 Author association lookup with a token, no credit without one; proof: a wiremock test on the build host
- [x] 1.3 `changelog_sections_with` passes `pull_request` and `commit`; proof: an integration test renders `([#12](…))` and the commit fallback, on the build host
- [x] 1.4 Operator instructions describe the rule; proof: `cargo test operator_docs` on the build host

2026-10-12 (UTC): on the build host at fbee308, `cargo clippy --keep-going --all-targets --all-features -- -D warnings` reported nothing and `mise run verify` passed: `a_squash_subject_names_its_pull_request`, `credits_only_outside_people`, `pull_author_reads_login_and_association`, `prepare_links_the_pull_request_or_commit_that_added_each_fragment`, and `operator_docs`. PR CI on #117 passed Plan, verify (with `semver`), and Template.
