# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. The operator reviewed this part with
[reusable-release-workflow](../reusable-release-workflow/tasks.md), task 0.1,
before it was split out.

## 1. Idempotent publish

- [x] 1.4 `verctl docs` and the `docs` job.
  Proof: the docs-build scenarios pass against two mocked docs.rs servers.
  One answers 404 or `doc_status` false, then `doc_status` true.
  The other answers false until the timeout.
  - 2026-10-17 (UTC): [#165][pr-165] shipped `verctl docs` ([CI run 39069206138][run-165]).
  - 2026-10-17 (UTC): the 0.4.6 release's `docs` job failed in [run 39075339999][run-046].
    It treated docs.rs's `doc_status: false` for a running build as a failed build.
    [#171][pr-171] keeps polling on false and adds `a_running_build`.
    The rerun of that job reported `verctl 0.4.6 built`.
    Not covered by a test: the tag lookup in `released_crates`, which only the live run exercised.

[pr-165]: https://github.com/victor-software-house/verctl/pull/165
[pr-171]: https://github.com/victor-software-house/verctl/pull/171
[run-046]: https://github.com/victor-software-house/verctl/actions/runs/39075339999
[run-165]: https://github.com/victor-software-house/verctl/actions/runs/39069206138
