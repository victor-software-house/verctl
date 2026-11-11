# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host, not the authoring workstation.

## 1. The fix

- [ ] 1.1 `git::file_at(root, sha, path)` reads one file at a commit, or `None` when the commit lacks it.
  Verify: a `git` test reads a file at its first commit and gets `None` at a commit before it.
- [ ] 1.2 `pending` keeps HEAD's targets for a release that holds every asset, and otherwise keeps only the ids `.ctl/ver.yaml` declared at the release commit. Earlier unreleased versions use their own commits.
  Verify: `pending` tests for a target added after a release, a target the release commit declared, and a commit with no `.ctl/ver.yaml`.

## 2. Bundle

- [ ] 2.1 `verctl instructions` and the operator skill say which assets a release owes, through their templates.
  Verify: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` on the build host leaves no diff on a second run.
- [ ] 2.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.
