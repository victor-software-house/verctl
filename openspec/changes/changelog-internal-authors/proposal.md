# List the maintainers a changelog never credits

## Why

Release notes credit no maintainer and credit outside contributors only.
verctl 0.3.15 writes no byline at all, because nothing told it who a
maintainer is. Its first attempt asked GitHub: the Version PR's App token
cannot see private organization membership, so `author_association` read
the repository owner as `CONTRIBUTOR` and credited them (Version PR #118).
The operator chose an explicit list as the control.

Queue row: VER-054. Lands on `next` for 0.4.0, because a new `Config` field
is a breaking library change.

## What changes

1. `.ctl/ver.yaml` accepts `changelog.internal_authors`, a list of GitHub
   logins never credited.
2. With a token, an entry credits its pull request author only when the
   login is not listed, the author is a person (`type` `User`), and the pull
   request comes from a fork.
3. `Config` and every config struct become `#[non_exhaustive]`, so later
   keys are additive.
4. Pull requests into `next` run cargo-semver-checks with
   `--release-type=major`, its name for the 0.3 to 0.4 step.

## Impact

1. `src/config.rs`, `src/publisher.rs`, `src/github.rs`, `src/release.rs`,
   `src/main.rs`, `.github/workflows/ci.yml`, tests, operator docs.
2. The six ctl repositories list their maintainers and Apps after 0.4.0.
