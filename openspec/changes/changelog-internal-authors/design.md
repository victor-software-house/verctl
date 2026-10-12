# Design

## Decisions

1. **An explicit list decides who is a maintainer.** Alternative:
   GitHub's `author_association`. Lost: an App token reads a private
   organization member as `CONTRIBUTOR`, which credited the owner on
   Version PR #118.
2. **A byline also needs a person and a fork.** A bot (`type` `Bot`) is
   never credited, and neither is a pull request whose head branch lives in
   the repository itself. The fork test is an extra condition, never the
   only one: a listed maintainer is not credited from a fork either.
   Alternative: the list alone. Lost: an unlisted maintainer pushing a
   branch to the repository would be credited.
3. **No token, or a failed lookup, credits no one.** The reference is still
   written.
4. **`#[non_exhaustive]` on every config struct.** Every field of `Config`
   and its nested structs is public, so each new key was a breaking change
   for cargo-semver-checks. Marking them once, in 0.4.0, makes later keys
   additive. Enums stay exhaustive: matching on every variant is the point.
   Alternative: private fields with accessors. Lost: every reader changes.
5. **Pull requests into `next` skip the Semver step.** `next` carries
   0.4.0's breaking changes while `Cargo.toml` still says 0.3.x, so a plain
   `cargo semver-checks` fails every one of them. cargo-semver-checks follows
   Cargo's semver, where 0.3 to 0.4 is the major step:
   `--release-type=minor` still fails, and `--release-type=major` skips all
   260 checks. Alternative: pass `--release-type=major`. Lost: it reads as a
   check and checks nothing. The check resumes on main against 0.4.0. Alternative: skip
   semver on `next`. Lost: an unintended break would go unseen until 0.4.0.

## Override and disable

- `changelog.internal_authors` is the override; omitting it lists no one,
  so a fork pull request by a person is credited.
- There is no switch to credit maintainers; the house rule forbids it.
