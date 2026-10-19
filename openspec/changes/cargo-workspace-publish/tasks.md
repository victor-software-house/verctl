# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Members

- [x] 1.1 `members::members` lists a virtual workspace's publishing members in dependency order through `cargo metadata`; proof: `cargo_metadata_lists_the_publishing_members`, `a_dependent_listed_first_publishes_after_its_dependency`, `a_path_only_dev_dependency_does_not_order_publishes`, and `a_member_with_publish_false_is_left_out` pass in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.
- [x] 1.2 A cycle among dependencies that order publishes fails and names it; proof: `versioned_dev_dependencies_both_ways_are_a_cycle` passes in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.

## 2. Publish

- [x] 2.1 A workspace package uploads each member with `--package` under its own index check, and an override keeps one command; proof: `a_virtual_workspace_uploads_each_member_against_the_index` and `an_overridden_workspace_upload_stays_one_command` pass in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.
- [x] 2.2 `publish --dry-run` lists the members in publish order; proof: `dry_run_lists_workspace_members_in_publish_order` passes in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.
- [x] 2.3 A 429 names the member and the retry time and fails before any tag; proof: `a_rate_limit_names_the_crate_and_the_retry_time` and `a_rate_limit_without_a_time_still_says_so` pass in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.
- [ ] 2.4 `pending` counts a workspace package complete only when every member is listed, and `docs` waits for every library member; proof: code review of `registry_lists` and `released_crates`, then the first workspace release reports each member

## 3. Docs

- [x] 3.1 The operator instructions name the mode and the rate-limit stop; proof: `cargo test operator_docs` passes in CI
  - 2026-10-19 (UTC): passed in [verify run 39185946178](https://github.com/victor-software-house/verctl/actions/runs/39185946178) on #174.
