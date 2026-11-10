# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Config

- [ ] 1.1 `Config` gains a `proposal` block with `title` and `commit_message`, and `Prepare` refuses unknown keys.
  Proof: config tests for an empty file, a title only, and both keys pass in CI
- [ ] 1.2 `prepare.version_label` fails to load with a message that names it and the fix of deleting it.
  Proof: a config test asserts the message and its line

## 2. Version PR

- [ ] 2.1 `VersionPr` reads the two keys and no variable; proof: a test sets `VERCTL_PR_TITLE` and asserts the default title
- [ ] 2.2 `open_or_update_pr` applies no label, and `github::ensure_version_label` is removed; proof: `rg ensure_version_label src` prints nothing

## 3. Workflow and action

- [ ] 3.1 `release.yml` and `actions/version-pr` drop `title` and `commit-message`, and this repository's caller passes neither; proof: `mise run lint:workflows` passes in CI
- [ ] 3.2 Each `release.yml` job that runs verctl checks `verctl --version` against 0.8.0 first.
  Proof: the step fails a run whose PATH verctl prints 0.7.4, run by hand once in a scratch caller
- [ ] 3.3 No spec declares the removed inputs; proof: `rg 'commit-message|VERCTL_PR_TITLE|VERCTL_COMMIT_MESSAGE' openspec/specs/ openspec/changes/ --glob '**/spec.md' --glob '!archive/**'` prints only the requirements that state the removal

## 4. Bundle

- [ ] 4.1 `src/instructions.md`, `skills/verctl/SKILL.md`, `README.md`, `AGENTS.md`, and rustdoc name the keys and no removed name; proof: `rg 'VERCTL_PR_TITLE|VERCTL_COMMIT_MESSAGE|version_label|commit-message'` outside `CHANGELOG.md` and archived changes prints nothing
- [ ] 4.2 A `minor` fragment lists the migration.
  - `VERCTL_PR_TITLE` and the `title` input map to `proposal.title`.
  - `VERCTL_COMMIT_MESSAGE` and the `commit-message` input map to `proposal.commit_message`.
  - `prepare.version_label` is removed.
  - The verctl pin moves with the workflow ref.

  Proof: the fragment in the pull request
