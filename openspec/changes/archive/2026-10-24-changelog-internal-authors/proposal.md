# List the maintainers a changelog never credits

## Why

Release notes credit no maintainer and credit outside contributors only.
verctl 0.3.15 writes no byline at all, because it has no list of maintainers.

Its first attempt used GitHub.
The Version PR's App token cannot see private organization membership.
So `author_association` read the repository owner as `CONTRIBUTOR`
and credited them (Version [PR 118][verctl#118]).
The operator chose an explicit list as the control.

Queue row: [VER-054][ver-054]. Lands on `next` for 0.4.0, because a new `Config` field
is a breaking library change.

## What changes

1. `.ctl/ver.yaml` accepts `changelog.internal_authors`, a list of GitHub
   logins never credited.
2. With a token, an entry credits its pull request author only when all
   three hold:
   - the login is not listed
   - the author is a person (`type` `User`)
   - the pull request comes from a fork
3. `Config` and every config struct become `#[non_exhaustive]`, so later
   keys are additive.
4. Pull requests into `next` skip the Semver step.

## Impact

1. `src/config.rs`, `src/publisher.rs`, `src/github.rs`, `src/release.rs`,
   `src/main.rs`, `.github/workflows/ci.yml`, tests, operator docs.
2. The six ctl repositories list their maintainers and Apps after 0.4.0.

[verctl#118]: https://github.com/victor-software-house/verctl/pull/118
[ver-054]: ../../../../tasks.yaml
