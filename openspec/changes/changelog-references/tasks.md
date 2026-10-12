# Tasks

- [ ] 1.2 First-parent commit that added a fragment, and the `(#N)` subject parse; proof: unit tests on a temporary repository, on the build host
- [ ] 1.3 Author association lookup with a token, no credit without one; proof: a wiremock test on the build host
- [ ] 1.4 `changelog_sections_with` passes `pull_request` and `commit`; proof: an integration test renders `([#12](…))` and the commit fallback, on the build host
- [ ] 1.5 Operator instructions describe the rule; proof: `cargo test operator_docs` on the build host
