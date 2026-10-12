## ADDED Requirements

### Requirement: Each changelog entry links its pull request or commit

verctl SHALL link each
changelog entry to the pull request named by a trailing `(#N)` in the
subject of the first-parent commit that added its fragment, and SHALL link
that commit when the subject names no pull request.

#### Scenario: Squash-merged fragment

- **WHEN** `.changeset/fix.md` was added by commit `feat: fix the parser (#12)` in a repository whose `origin` is `https://github.com/acme/demo.git`, and `verctl prepare --dry-run` runs
- **THEN** the entry reads `- Fix the parser ([#12](https://github.com/acme/demo/pull/12)).`

#### Scenario: Fragment pushed directly

- **WHEN** `.changeset/fix.md` was added by commit `abc1234` with subject `fix: the parser`, and `verctl prepare --dry-run` runs
- **THEN** the entry ends with `` ([`abc1234`](https://github.com/acme/demo/commit/<sha>)) ``

### Requirement: Only outside contributors are credited

verctl SHALL credit an entry's author only when GitHub reports the pull
request author's `author_association` as other than `OWNER`, `MEMBER`, or
`COLLABORATOR` and the author's `type` as `User`. Without a token it SHALL
credit no one.

#### Scenario: Maintainer pull request

- **WHEN** pull request #12 was opened by a login whose `author_association` is `OWNER`
- **THEN** the entry carries no byline

#### Scenario: Outside contributor

- **WHEN** pull request #12 was opened by `octo`, a `User` with `author_association` `CONTRIBUTOR`
- **THEN** the entry reads `([#12](https://github.com/acme/demo/pull/12) by [@octo](https://github.com/octo))`

#### Scenario: Bot author

- **WHEN** pull request #12 was opened by `renovate[bot]`, a `Bot` with `author_association` `NONE`
- **THEN** the entry carries no byline

#### Scenario: No token

- **WHEN** `verctl prepare --dry-run` runs without `GITHUB_TOKEN`
- **THEN** every entry keeps its reference and carries no byline
