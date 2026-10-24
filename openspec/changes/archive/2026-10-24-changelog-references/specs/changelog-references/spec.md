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

### Requirement: No entry credits an author

verctl SHALL write no author byline on any changelog entry, so no
maintainer is credited.

#### Scenario: Maintainer pull request read as a contributor

- **WHEN** pull request #12 was opened from a branch in `acme/demo` by a member the App token reads as `CONTRIBUTOR`, and `verctl prepare --pr` runs
- **THEN** the entry reads `([#12](https://github.com/acme/demo/pull/12))` with no byline
