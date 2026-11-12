# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The cap

- [ ] 1.1 `release::pr_body` returns a body of at most 65,536 characters at any changelog size, and leaves a shorter changelog as it was.
  Verify: the `release` tests `pr_body_at_the_limit_is_unchanged` and `pr_body_is_the_changelog` pass in CI.
- [ ] 1.2 A longer changelog keeps every whole line that fits, ends with the note, and holds valid UTF-8 at the cut.
  Verify: the `release` tests `pr_body_over_the_limit_keeps_the_bullets_that_fit` and `pr_body_cut_keeps_whole_multibyte_characters` pass in CI.
- [ ] 1.3 The cut never lands inside a fenced code block and never leaves a heading last.
  Verify: the `release` tests `pr_body_cut_never_lands_inside_a_fenced_code_block`, `pr_body_cut_drops_a_heading_left_at_the_end`, and `pr_body_with_no_safe_cut_is_only_the_note` pass in CI.

## 2. Bundle

- [ ] 2.1 The instructions and their template state the cut.
  Verify: the `operator_docs` test passes in CI.
- [ ] 2.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.
