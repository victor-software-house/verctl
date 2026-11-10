# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md).
That change records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 3. The Version PR

- [ ] 3.1 `propose` finds its pull request by head repository, `verctl/release/<base>`, and base. It opens, updates, and closes the pull request, and refuses while a release is incomplete. The `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`. Proof: the version-proposal identity and lifecycle scenarios pass
  - 2026-11-09, partly shipped: `prepare --pr` uses the derived branch `verctl/release/<base>` ([#161][pr161]). Remaining: the `propose` verb, the refusal while a release is incomplete, and `proposal` keys in place of `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`, which shipped as the interface in [#155][pr155].
  - 2026-11-09, the `proposal` keys are in 0.8.0 through [version-pr-proposal](../version-pr-proposal/proposal.md) (queue row [VER-090][ver-090]). The `propose` verb and the refusal stay for later.
- [ ] 3.2 `check` enforces identity and content; the label, the branch exemption, and the `labeled` and `unlabeled` CI types are removed; proof: the version-proposal `check` scenarios pass
  - 2026-11-09, partly shipped: `check` identifies the Version PR by identity, reads no label, and CI dropped the `labeled` and `unlabeled` types ([#188][pr188]). Remaining: the content check, and removing `prepare.version_label`, which still labels the pull request.
  - 2026-11-09, the label is removed in 0.8.0 through [version-pr-proposal](../version-pr-proposal/proposal.md) (queue row [VER-090][ver-090]); the content check stays for later.
  - 2026-11-10, the content check and the two rules for other pull requests ship in 0.8.0. They ship through [version-pr-content](../version-pr-content/tasks.md) (queue row [VER-095][ver-095]), tasks 2.3 and 2.4. The two rules cover a CHANGELOG section and a change to whether a package publishes.

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[pr155]: https://github.com/victor-software-house/verctl/pull/155
[pr161]: https://github.com/victor-software-house/verctl/pull/161
[pr188]: https://github.com/victor-software-house/verctl/pull/188
[ver-090]: ../../../tasks.yaml
[ver-095]: ../../../tasks.yaml
