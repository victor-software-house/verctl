# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Check

- [ ] 1.1 `check` renders every template in memory and lists each differing target with its template, with or without fragments; proof: integration tests for a hand-edited target, a matching tree, and an empty `.changeset/` pass in CI
- [ ] 1.2 `check` fails on a pin whose pattern no longer matches or whose file names another version; proof: integration tests for both cases pass in CI

## 2. One owner

- [ ] 2.1 Loading the config fails when a template's target is a `pins` file, naming all three; proof: a test asserts the message, and `version` never reaches the write

## 3. Bundle

- [ ] 3.1 `docs/served-files.md` section 6, `src/instructions.md`, `skills/verctl/SKILL.md`, and `AGENTS.md` say what `check` verifies; proof: the diff, and `operator_docs` passes in CI
- [ ] 3.2 A `minor` fragment says `check` now fails on a hand-edited served file, a stale pin, and a file with two owners; proof: the fragment in the pull request
- [ ] 3.3 This repository's own `check` passes on the pull request that ships the change; proof: the `verify` run
