# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The cap

- [x] 1.1 `release::pr_body` returns a body of at most 65,536 characters at any changelog size, and leaves a shorter changelog as it was.
  Verify: the `release` tests `pr_body_at_the_limit_is_unchanged` and `pr_body_is_the_changelog` pass in CI.
  - 2026-11-12 (UTC): both tests print `ok` in the `verify` job of [CI run 43172384351][ci-run] at `c7a0ed7`, on [PR 351][pr-351].
- [x] 1.2 A longer changelog keeps every whole line that fits, ends with the note, and holds valid UTF-8 at the cut.
  Verify: the `release` tests `pr_body_over_the_limit_keeps_the_bullets_that_fit` and `pr_body_cut_keeps_whole_multibyte_characters` pass in CI.
  - 2026-11-12 (UTC): both tests print `ok` in [CI run 43172384351][ci-run].
- [x] 1.3 The cut never lands inside a fenced code block and never leaves a heading last.
  Verify: the `release` tests `pr_body_cut_never_lands_inside_a_fenced_code_block`, `pr_body_cut_drops_a_heading_left_at_the_end`, and `pr_body_with_no_safe_cut_is_only_the_note` pass in CI.
  - 2026-11-12 (UTC): all three tests print `ok` in [CI run 43172384351][ci-run].

## 2. Bundle

- [x] 2.1 The instructions and their template state the cut.
  Verify: the `operator_docs` test passes in CI.
  - 2026-11-12 (UTC): `operator_docs::instructions_are_the_committed_surface_render` and `operator_docs::skill_is_the_committed_surface_render` print `ok` in [CI run 43172384351][ci-run].
- [x] 2.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.
  - 2026-11-12 (UTC): the `check:versions` task, which runs `cargo run -- check --versions`, passed inside the `verify` job of [CI run 43172384351][ci-run].
  - Not verified: the changeset's rendered changelog entry, which appears only in the Version PR.

[ci-run]: https://github.com/victor-software-house/verctl/actions/runs/43172384351
[pr-351]: https://github.com/victor-software-house/verctl/pull/351
