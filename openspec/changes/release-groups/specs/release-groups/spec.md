## ADDED Requirements

### Requirement: A group shares one version

`groups` SHALL name sets of packages that share one version.
A fragment that bumps one member SHALL bump every member to the largest bump in the group.
A package in no group SHALL move on its own.

#### Scenario: One member's fragment moves the group

- **WHEN** group `cli` holds `verctl` and `verctl-core`, both at `1.2.0`, and `.changeset/feat.md` holds `verctl-core: minor`
- **THEN** `verctl version` moves both to `1.3.0`

### Requirement: Changesets groups are read when none are declared

With no `groups` key, verctl SHALL read `fixed` and `ignore` from `.changeset/config.json` when that file exists.
A `linked` entry SHALL fail the load and name the `groups` entry to write instead.

#### Scenario: A repository moving from Changesets

- **WHEN** `.changeset/config.json` holds `"fixed": [["a", "b"]]` and `.ctl/ver.yaml` has no `groups`
- **THEN** a fragment bumping `a` also bumps `b`

### Requirement: The default tag shape follows the package count

A repository with one package and no group SHALL default to the tag `v{version}`.
A repository with more than one package or group SHALL default to `{name}/v{version}`.
`tags.template` SHALL override either.

#### Scenario: Two packages and no template

- **WHEN** packages `cli` and `core` release `1.4.0` and `0.2.1`, and `.ctl/ver.yaml` sets no `tags.template`
- **THEN** `verctl publish` tags `cli/v1.4.0` and `core/v0.2.1`

### Requirement: Adoption keeps the existing tag shape

When a package already has release tags, verctl SHALL keep the shape of the newest one that parses as a version, so adoption never retags.
A tag shape that `tags.template` cannot match SHALL fail `check` and name a template that fits.

#### Scenario: A repository tagged by hand

- **WHEN** the newest tag is `v0.0.75` and the repository now declares two packages
- **THEN** the next release of the package that owns those tags is tagged `v0.0.76`

### Requirement: A Go module in a subdirectory is tagged by its directory

A `go.mod` package outside the root SHALL be tagged `{dir}/v{version}`, the only form the Go toolchain reads, and a template that gives it another tag SHALL fail the load.

#### Scenario: A nested Go module

- **WHEN** package `tools` has `path: tools/go.mod` and releases `0.3.0`
- **THEN** `verctl publish` tags `tools/v0.3.0`

### Requirement: A group gets one tag and one Release

A group SHALL get one tag and one Release, whose body holds each member's CHANGELOG section.

#### Scenario: Releasing a group

- **WHEN** group `cli` releases `1.3.0`
- **THEN** `verctl publish` creates the tag `cli/v1.3.0` and one Release holding both members' sections

### Requirement: Floating major tags are opt-in

`tags.floating: [major]` SHALL move a lightweight `v<major>` tag, or `{name}/v<major>`, to each new release in that major.
No Release SHALL name a floating tag.
A floating tag is not a release tag: the annotated-only rule and the release-tag ruleset do not cover it, so `publish` can move it while `tags.immutable` is `true`.

#### Scenario: A reusable workflow consumers pin by major

- **WHEN** `tags.floating: [major]` is set and package `ci` releases `1.2.1` after `1.2.0`
- **THEN** `verctl publish` moves `ci/v1` to the `1.2.1` commit and creates no Release for it
