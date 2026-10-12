# Tasks

- [x] 1.1 First-parent commit that added a fragment, and the `(#N)` subject parse; proof: unit tests on a temporary repository, on the build host
- [x] 1.2 No byline on any entry; proof: the integration test asserts the bare `([#12](…))`, on the build host
- [x] 1.3 `changelog_sections_with` passes `pull_request` and `commit`; proof: an integration test renders `([#12](…))` and the commit fallback, on the build host
- [x] 1.4 Operator instructions describe the rule; proof: `cargo test operator_docs` on the build host

2026-10-12 (UTC): on the build host at fbee308, `cargo clippy --keep-going --all-targets --all-features -- -D warnings` reported nothing and `mise run verify` passed: `a_squash_subject_names_its_pull_request`, `credits_only_outside_people`, `pull_author_reads_login_and_association`, `prepare_links_the_pull_request_or_commit_that_added_each_fragment`, and `operator_docs`. PR CI on #117 passed Plan, verify (with `semver`), and Template.

2026-10-12 (UTC): verctl 0.3.15's Version PR #118 credited the repository owner: the App token read a private member as `CONTRIBUTOR`. The author lookup is removed, so no entry carries a byline; VER-054 adds `changelog.internal_authors` on `next`.
