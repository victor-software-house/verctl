# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md).
That change records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 6. Patches after 0.8.0

- [ ] 6.3 The `bootstrap` stage stops a first publication under OIDC; proof: the release-completion bootstrap scenario passes
  - 2026-11-09, not started. Task 6.5 of [built-in-ecosystems](../built-in-ecosystems/tasks.md) covers the detection this needs. It also replaces the stop with a token publish when the caller passes a token.

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
