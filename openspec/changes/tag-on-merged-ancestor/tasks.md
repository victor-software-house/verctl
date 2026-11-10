# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Tag lookup

- [x] 1.1 `tagged_commit` accepts a tagged commit on a merge's second parent at the version HEAD has, with the merge's depth; proof: `a_tag_on_a_merged_commit_at_the_same_version_is_the_release` passes in CI
- [x] 1.2 A merged commit at another version, and a commit HEAD does not contain, still return no release; proof: `a_tag_on_a_merged_commit_at_another_version_is_not_the_release` and `a_tag_on_a_commit_head_does_not_contain_is_not_the_release` pass in CI
- [ ] 1.3 The serde-saphyr fork plans past its `v0.0.1` tag on a release of this fix; proof: its `Fork publish` plan job succeeds

## 2. Docs

- [x] 2.1 The rustdoc of `tagged_commit` and `pending`, and the design of `release-at-version-commit`, split from `reusable-release-workflow`, state the rule; proof: `rg -n 'off the first-parent' src openspec/changes/release-at-version-commit` finds no match
- [x] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
