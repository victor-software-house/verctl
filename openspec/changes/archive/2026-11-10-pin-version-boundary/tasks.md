# Tasks

Compile, format, lint, and test tasks run in pull request CI with `mise run verify`, or before a push in an ephemeral box on the build host.

## 1. Refuse

- [x] 1.1 `Config::load` refuses a pattern without a boundary on either side, naming the id and the side.
  Proof: config tests pass in CI for each case.
  The cases are the accepted and refused patterns in docs/served-files.md section 4.
  - 2026-11-10 (UTC): `a_pattern_marks_where_the_version_starts_and_ends` and `a_digit_next_to_the_version_leaves_the_side_open` cover every row of that table.
    Both passed in CI on [#290][pr290] ([verify][run290]).
- [x] 1.2 A prerelease in a pinned file stops `version` and leaves the file unchanged. Proof: an integration test passes in CI
  - 2026-11-10 (UTC): `a_prerelease_pin_stops_version_before_any_write` in `tests/served.rs` passed in CI on [#281][pr281] ([verify][run281]) and [#290][pr290] ([verify][run290]).
- [x] 1.3 A `tool` pin whose include ref is `v1.2.3-rc1` fails and leaves the file unchanged.
  Proof: the test fails on main and passes with the fix, both run in a build-host box.
  - 2026-11-10 (UTC): `an_include_with_a_prerelease_ref_fails_instead_of_half_rewriting` failed in a box on the tree before the fix, with `unwrap_err` called on an `Ok`.
    With the fix, `mise run verify` passed in a box, then in CI on [#281][pr281] ([verify][run281]).
- [x] 1.4 The config fixtures, `tests/served.rs`, and `examples/ver.yaml` use bounded patterns. Proof: `mise run verify` passes
  - 2026-11-10 (UTC): `mise run verify` passed in CI on [#281][pr281] ([verify][run281]), [#285][pr285] ([verify][run285]), and [#290][pr290] ([verify][run290]).

## 2. Bundle

- [x] 2.1 `docs/served-files.md` section 4, `examples/ver.yaml`, `src/instructions.md`, `skills/verctl/SKILL.md`, and the `PinPattern` rustdoc state the rule.
  Proof: the diff, and `rg 'Give the version a right-hand boundary'` finds nothing.
  - 2026-11-10 (UTC): the diffs of [#281][pr281], [#285][pr285], and [#290][pr290] state the rule in each of those files.
    The `rg` search matches only this task's own proof line.
- [x] 2.2 A `minor` fragment names the refusal and the two fixes. Proof: the fragment in the pull request
  - 2026-11-10 (UTC): [the fragment][fragment] is `verctl: minor`.
    It names the refusal, the two fixes (text in `match`, or `whole_line: true`), and the `tool` ref failure.
- [x] 2.3 design.md section 4 lists the caller edits for qctl, forkctl, and rgbctl.
  Their 0.8.0 adoption PRs make them; proof: the table
  - 2026-11-10 (UTC): [design.md][design] section 4 lists six edits.
    They match each repository's `.ctl/ver.yaml` on its default branch, read on that date.

[pr281]: https://github.com/victor-software-house/verctl/pull/281
[pr285]: https://github.com/victor-software-house/verctl/pull/285
[pr290]: https://github.com/victor-software-house/verctl/pull/290
[run281]: https://github.com/victor-software-house/verctl/actions/runs/42809596720/job/129808000635
[run285]: https://github.com/victor-software-house/verctl/actions/runs/42812030843/job/129816256007
[run290]: https://github.com/victor-software-house/verctl/actions/runs/42821014227/job/129846919576
[fragment]: https://github.com/victor-software-house/verctl/blob/2f96b49de937694538147e796395da500887485a/.changeset/pin-version-boundary.md
[design]: design.md
