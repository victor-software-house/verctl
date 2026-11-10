# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Check

- [x] 1.1 `check` renders every template in memory and lists each differing target with its template, with or without fragments.
  Proof: integration tests for a hand-edited target, a matching tree, and an empty `.changeset/` pass in CI.
  Evidence, 2026-11-10: shipped in [#276][verctl#276]. In `tests/served.rs`, whose fixture holds an empty `.changeset/`, `check_passes_when_every_served_file_matches` and `render_reverts_a_hand_edited_served_file` pass.
  `a_served_file_edited_by_hand_fails` in `tests/content.rs` covers the case with a fragment.
- [x] 1.2 `check` fails on a pin whose pattern no longer matches or whose file names another version.
  Proof: integration tests for both cases pass in CI.
  Evidence, 2026-11-10: `check_names_a_stale_pin` covers a pin at another version and a pattern that matches 0 times ([#276][verctl#276]).

## 2. One owner and one fix

- [x] 2.1 `version`, `check`, and `render` fail when a template's target is a `pins` file, naming all three.
  Proof: a test asserts the message for each verb, and `version` writes nothing.
  Evidence, 2026-11-10: `a_file_with_two_owners_is_refused_before_any_write` asserts the message for `version`, `check` and `render`, and that `version` leaves `Cargo.toml` and `tools.toml` unchanged.
- [x] 2.2 `render` rewrites the pins, then renders every template, and a failed `check` names it.
  Proof: integration tests for a hand edit and for a template edit with no fragment pass in CI.
  Evidence, 2026-11-10: `render_reverts_a_hand_edited_served_file` and `a_template_edit_fails_check_until_render` pass ([#276][verctl#276]).

## 3. Bundle

- [x] 3.1 `docs/served-files.md` section 6, `src/instructions.md`, `skills/verctl/SKILL.md`, `AGENTS.md`, and `render`'s help cover what `check` verifies and what `render` writes.
  Proof: the diff, and `operator_docs` passes in CI.
  Evidence, 2026-11-10: [#276][verctl#276] changed `docs/served-files.md` section 6, the instructions, the skill and `AGENTS.md`, and `operator_docs` passed.
- [x] 3.2 A `minor` fragment lists the changes.
  `check` now fails on a hand-edited served file, a stale pin, and a file with two owners.
  `render` renders templates.
  Proof: the fragment in the pull request.
  Evidence, 2026-11-10: the [#276][verctl#276] fragment is in `CHANGELOG.md` under verctl 0.8.0.
- [x] 3.3 This repository's own `check` passes on the pull request that ships the change; proof: the `verify` run.
  Evidence, 2026-11-10: `verify` passed on [#276][verctl#276] in [run 42786032629][verify-276].

[verctl#276]: https://github.com/victor-software-house/verctl/pull/276
[verify-276]: https://github.com/victor-software-house/verctl/actions/runs/42786032629
