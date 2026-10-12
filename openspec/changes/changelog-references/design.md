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
3. **No byline until maintainers are listed.** The rule is to credit no
   maintainer, and only an explicit list says who one is. Alternative:
   credit a pull request author whose `author_association` is not `OWNER`,
   `MEMBER`, or `COLLABORATOR`. Lost: the App token behind a Version PR
   cannot see private organization membership and reads a member as
   `CONTRIBUTOR`; verctl 0.3.15's first Version PR (#118) credited the
   repository owner that way. A fork check is a heuristic too: a maintainer
   can push from a fork.
4. **Links point at the `origin` remote.** URLs use the repository
   `origin` names, which `actions/checkout` sets. A checkout without a GitHub
   `origin` gets no references. Alternative: `github::repo`, which prefers
   `GITHUB_REPOSITORY`. Lost: a fixture repository without a remote would
   render links in CI and none locally.
5. **No configuration key in this patch.** `Config` has only public fields
   and is not `#[non_exhaustive]`, so cargo-semver-checks reports a new
   field as a breaking change, which a patch cannot carry. Alternative: ship
   the whole change on `next` with 0.4.0. Lost: consumers would wait for
   every reference until 0.4.0.

## Override and disable

- In this patch there is no switch: references are always written and no
  byline is. A commit subject without `(#N)` falls back to the commit link.
- 0.4.0 adds `changelog.internal_authors` (logins never credited), with
  `Config` and its nested structs `#[non_exhaustive]` so later keys stay
  additive. A byline then needs an author outside that list and not a bot.
  VER-054 carries it on `next`.
