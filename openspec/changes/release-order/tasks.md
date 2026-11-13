# Tasks

## 1. Release

- [x] 1.1 Order: draft and assets, registries, then the draft published with its tag. Proof: a CI test that records the calls in order.
- [x] 1.2 A partial release is a `status` state with its cause. Proof: a snapshot test of the human and JSON views.
- [x] 1.3 A rerun skips every crate the index lists. Proof: a CI test against a recorded index with one of three crates live.
- [x] 1.4 The tag proof reads the release's own tag again on 404, at most 5 reads in total within 30 seconds, for a run and for a rerun, and fails at once on any other error. Proof: a CI test against recorded responses where the first read is a 404, as in [rawr run 43414046487][rawr-run].

## 2. Documents

- [x] 2.1 Update AGENTS.md, the instructions, the operator skill and the README for the order and the partial state. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
  - The instructions, the skill and the README came with [#377][pr-377] and [#378][pr-378], whose CI ran `operator_docs`; AGENTS.md follows in this task's pull request.
- [x] 2.2 A `patch!` fragment for verctl lists the later tag under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.
  - [Version PR #384][pr-384] listed it under `### Breaking`, and [run 43511698400][release-run] released `0.8.6-rc.2` in that order: the tag `v0.8.6-rc.2` names the merge commit `19ee926`, and crates.io lists the version through trusted publishing.

[rawr-run]: https://github.com/victor-software-house/rawr/actions/runs/43414046487
[pr-377]: https://github.com/victor-software-house/verctl/pull/377
[pr-378]: https://github.com/victor-software-house/verctl/pull/378
[pr-384]: https://github.com/victor-software-house/verctl/pull/384
[release-run]: https://github.com/victor-software-house/verctl/actions/runs/43511698400
