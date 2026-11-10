# Tasks

Compile, format, lint, and test tasks run in pull request CI with `mise run verify`, or before a push in an ephemeral box on the build host.

## 1. Refuse

- [ ] 1.1 `Config::load` refuses a pattern without a boundary on either side, naming the id and the side.
  Proof: config tests pass in CI for each case.
  The cases are a missing right side, a missing left side, a `.` or a digit next to `{version}`, a lone `-` or `+`, and a `whole_line` pattern.
- [ ] 1.2 A prerelease in a pinned file stops `version` and leaves the file unchanged; proof: an integration test passes in CI
- [ ] 1.3 A `tool` pin whose include ref is `v1.2.3-rc1` fails and leaves the file unchanged.
  Proof: the test fails on main and passes with the fix, both run in a build-host box.
- [ ] 1.4 The config fixtures, `tests/served.rs`, and `examples/ver.yaml` use bounded patterns; proof: `mise run verify` passes

## 2. Bundle

- [ ] 2.1 `docs/served-files.md` section 4, `examples/ver.yaml`, `src/instructions.md`, `skills/verctl/SKILL.md`, and the `PinPattern` rustdoc state the rule.
  Proof: the diff, and `rg 'Give the version a right-hand boundary'` finds nothing.
- [ ] 2.2 A `minor` fragment names the refusal and the two fixes; proof: the fragment in the pull request
- [ ] 2.3 design.md section 4 lists the caller edits for qctl, forkctl, and rgbctl.
  Their 0.8.0 adoption PRs make them; proof: the table
