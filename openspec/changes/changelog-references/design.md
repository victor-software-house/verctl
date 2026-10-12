# Design

## Decisions

1. **The pull request number comes from the commit subject.** verctl finds
   the first-parent commit that added the fragment file and reads a trailing
   `(#N)` from its subject, which is GitHub's squash and merge default.
   Alternative: ask GitHub which pull requests contain the commit
   (`GET /repos/{owner}/{repo}/commits/{sha}/pulls`). Lost: it needs a token
   for every entry, so a local `prepare --dry-run` would render differently
   from CI.
2. **No pull request number falls back to the commit.** A fragment added by
   a commit whose subject names no pull request links that commit,
   `` ([`abc1234`](url)) ``. Alternative: no reference. Lost: a direct
   push would leave the entry untraceable.
3. **Credit only outside contributors, decided by GitHub.** With a token,
   verctl reads the pull request (`GET /repos/{owner}/{repo}/pulls/{N}`)
   and credits its author only when `author_association` is not `OWNER`,
   `MEMBER`, or `COLLABORATOR` and the user `type` is `User`, which
   excludes bots. Without a token, or when the lookup fails, it credits no
   one. Alternative: credit everyone outside a configured list of internal
   logins. Lost: every maintainer would have to be listed in every
   repository, and a missing name credits a maintainer.
4. **Links point at the `origin` remote.** URLs use the repository
   `origin` names, which `actions/checkout` sets. A checkout without a GitHub
   `origin` gets no references. Alternative: `github::repo`, which prefers
   `GITHUB_REPOSITORY`. Lost: a fixture repository without a remote would
   render links in CI and none locally.
5. **No configuration key in this patch.** `Config` has only public fields
   and is not `#[non_exhaustive]`, so cargo-semver-checks reports a new
   field as a breaking change, which a patch cannot carry. Alternative: ship
   the keys on `next` with 0.4.0. Lost: consumers wait for every reference
   until 0.4.0 for a toggle no one has asked for.

## Override and disable

- In this patch there is no switch: references are always written, and only
  outside contributors who are people are credited. A commit subject without
  `(#N)` falls back to the commit link.
- 0.4.0 adds `changelog.references: false` (bare summaries) and
  `changelog.internal_authors` (logins never credited), with `Config`
  `#[non_exhaustive]` so later keys stay additive. VER-011 carries it.
