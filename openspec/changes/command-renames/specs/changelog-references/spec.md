## MODIFIED Requirements

### Requirement: Each changelog entry links a pull request or commit

verctl SHALL link each
changelog entry to a pull request.
A trailing `(#N)` in the subject of the first-parent commit that added its
fragment names that pull request.
verctl SHALL link that commit when the subject names no pull request.

#### Scenario: Squash-merged fragment

- **WHEN** `.changeset/fix.md` was added by a commit with subject `feat: fix the parser` and the trailing pull request reference `#12` in parentheses, in a repository whose `origin` is `https://github.com/acme/demo.git`, and `verctl version --dry-run` runs
- **THEN** the entry reads `- Fix the parser ([#12](https://github.com/acme/demo/pull/12)).`

#### Scenario: Fragment pushed directly

- **WHEN** `.changeset/fix.md` was added by commit `abc1234` with subject `fix: the parser`, and `verctl version --dry-run` runs
- **THEN** the entry ends with `` ([`abc1234`](https://github.com/acme/demo/commit/<sha>)) ``

### Requirement: No maintainer is credited

verctl SHALL write no author byline for a maintainer. Crediting an
outside contributor follows the `changelog-credit` capability.

#### Scenario: Maintainer pull request read as a contributor

- **WHEN** pull request 12 was opened from a branch in `acme/demo` by a member the App token reads as `CONTRIBUTOR`, and `verctl version --pr` runs
- **THEN** the entry reads `([#12](https://github.com/acme/demo/pull/12))` with no byline
