## ADDED Requirements

### Requirement: Every release tag is annotated and has a Release

`publish` SHALL create annotated tags only, and SHALL create a GitHub Release for every tag it makes, `publisher: none` packages included, so immutable Releases cover every tag.

#### Scenario: A GitHub-only package

- **WHEN** package `templates` has `publisher: none` and its Version PR has merged
- **THEN** `verctl publish` creates an annotated tag and a Release for it

### Requirement: A Release with assets stays a draft until they are uploaded

A Release that declares assets SHALL be created as a draft and published only after every asset is uploaded, because an immutable Release refuses later uploads.

#### Scenario: Assets built after the tag

- **WHEN** package `verctl` declares two asset targets and `publish` creates its Release
- **THEN** the Release stays a draft until both tarballs are uploaded, then is published

### Requirement: check reads the repository's immutability settings

`check --repository` SHALL report whether immutable Releases are on and whether a tag ruleset blocks updating and deleting release tags.
When `.ctl/ver.yaml` sets `tags.immutable: true`, a missing setting SHALL fail the check and name it.

#### Scenario: Immutability declared but off

- **WHEN** `tags.immutable` is `true` and the repository has immutable Releases off
- **THEN** `verctl check --repository` exits non-zero and names immutable Releases
