# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The mark

- [ ] 1.1 `PackageBump.breaking: bool`, read from `patch!` in `fragment::parse_str`. `minor!`, `major!`, and `none!` fail.
  Verify: `fragment` tests for `patch!`, for each refused value naming the file, and for a fragment that marks one package of two.

## 2. The semver check

- [ ] 2.1 `Decision::BreakingPatch`, chosen when the largest pending bump is a patch and a pending fragment marks the package. `presentation` labels it `skipped: patch! fragment`.
  Verify: `semver_check` tests for the mark, for a minor beside the mark, and for the same package with the fragment gone.

## 3. The changelog

- [ ] 3.1 `group_entries` puts a marked entry under `### Breaking`, first, in the marked package's section only.
  Verify: `release` tests compare whole sections, as `indoc!` fixtures, for both scenarios of the changelog-groups delta.

## 4. Bundle and release

- [ ] 4.1 `verctl instructions`, the operator skill, and `README.md` describe `patch!`, through their templates.
  Verify: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` on the build host leaves no diff on a second run, and the prose lint passes.
- [ ] 4.2 The fragment of this change and the publish options fragment mark `verctl: patch!`.
  Verify: the Semver step of the pull request's CI reports `verctl` as `skipped: patch! fragment` and passes.
