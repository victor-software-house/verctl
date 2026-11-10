# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Commands

- [ ] 1.1 `Command::Version(VersionArgs)` and `Command::Render(PublishArgs)` replace `Prepare` and `Pin`; proof: the cli parse tests use `version` and `render` and pass in CI
- [ ] 1.2 Hidden `prepare` and `pin` exit 2 and name the new verb, in the human and JSON error; proof: one integration test per removed verb asserts the exit code and the message
- [ ] 1.3 The report field `next` is `after` in JSON and in the human view; proof: the report snapshots and a JSON test show `after` and no `next`

## 2. Callers in this repository

- [ ] 2.1 `.github/workflows/release.yml`, `actions/version-pr/action.yml`, `.ctl/ver.yaml`, and `.ctl/operator/instructions.md.jinja` call `version` and `render`; proof: `rg --hidden '\b(ver|verctl) (prepare|pin)\b'` outside `CHANGELOG.md` and archived changes prints nothing, except the removed-verb scenarios and the deltas this change replaces
- [ ] 2.2 After `version-files`, `derived-version-pr-branch`, `version-pr-proposal`, and this change are archived, in that order, no spec names the old verbs. The archive also rewrites the `versioning` Purpose, which no delta can change, to say `version` where it says `prepare`. Proof: `rg -nw 'prepare|pin' openspec/specs/ openspec/changes/ --glob '**/spec.md' --glob '!**/archive/**'` prints only these matches:
  - the `prepare:` block and its `prepare.version_label` and `prepare.stages` keys in `version-pr-proposal`, which the config refusal names;
  - the noun "the verctl pin" in `version-pr-proposal`'s older-verctl requirement, which quotes the workflow error;
  - the requirement "A removed verb names its replacement" in `command-names`;
  - the assets step key `prepare` in `package-asset-files`' `asset-contents`;
  - the leftover `prepare:` section in `release-state-machine`'s `release-config`;
  - the `mise-pin` step in `built-in-ecosystems`.

## 3. Bundle

- [ ] 3.1 `src/instructions.md`, `skills/verctl/SKILL.md`, `README.md`, `AGENTS.md`, and rustdoc use the new names; proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` output is committed and `operator_docs` passes in CI
- [ ] 3.2 A `minor` fragment names `prepare` to `version`, `pin` to `render`, `next` to `after`, and the library renames; proof: the fragment in the pull request
