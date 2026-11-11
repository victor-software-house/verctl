# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

## 1. The workflow

- [ ] 1.1 The plan job writes `verctl-version` from its minimum check, and leaves it empty with `verctl-artifact`.
  Verify: `mise run lint:workflows` and `mise run lint:workflow-policy` pass in CI.
- [ ] 1.2 The publish, asset, and docs jobs run `mise exec ${VERCTL_TOOL:+"$VERCTL_TOOL"} -- verctl <verb>`.
  Verify: `rg 'mise run ver (release|docs)' .github/workflows/release.yml` finds nothing.
- [ ] 1.3 The header comment states that every job runs the plan's verctl.
  Verify: reading the header.

## 2. Proof

- [ ] 2.1 A caller releases a version commit that pins an older verctl than HEAD.
  Verify: the caller's run link, recorded here.
- [ ] 2.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.
