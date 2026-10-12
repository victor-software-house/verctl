# Merge the Version PR automatically once required checks pass

## Why

In the seven days to 2026-10-12, 13 Version PRs were merged by hand across
the six ctl repositories: ctl-core 6, forkctl 4, and verctl, qctl, and
yamled 1 each.
Each had one commit and green checks, so the merge added a wait and a click
and no review.
All six repositories already allow auto-merge.

Queue row: VER-047.

## What Changes

1. `actions/version-pr` gains an `auto-merge` input, `false` by default.
2. With `auto-merge: true`, after opening or updating the Version PR, the
   action enables GitHub auto-merge on it as a squash. The subject is the PR
   title and number, and the body is the PR description.
3. The action refuses to enable auto-merge when the base branch has no
   required status check, because GitHub would then merge at once with no
   gate.
4. This repository's Version PR workflow does the same.

## Capabilities

### New Capabilities

- `version-pr-auto-merge`: opting the Version PR into auto-merge behind the
  base branch's required checks.

## Impact

1. `actions/version-pr/action.yml`, `.github/workflows/version-pr.yml`,
   `examples/workflows/version-pr.yml`, README, and the operator
   instructions and skill.
2. Consumers opt in per repository with one input, after requiring their CI
   checks on the default branch.
