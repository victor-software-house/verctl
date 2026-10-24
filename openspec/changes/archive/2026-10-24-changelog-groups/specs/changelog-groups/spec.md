## ADDED Requirements

### Requirement: A fragment may name a changelog group

verctl SHALL read an optional `verctl:group` key from fragment frontmatter,
accept `added`, `changed`, and `fixed`, and fail on any other value.
The key SHALL NOT count as a package.

#### Scenario: Grouped fragment

- **WHEN** a fragment sets `demo: patch` and `verctl:group: fixed`
- **THEN** it bumps only `demo` and its entry belongs to the Fixed group

#### Scenario: Unknown group

- **WHEN** a fragment sets `verctl:group: security`
- **THEN** the parse fails with `unknown verctl:group "security"`

### Requirement: Grouped sections render under headings

A changelog section where no fragment names a group SHALL render as a flat
list, byte for byte as before.
A section where any fragment names a group SHALL render `### Added`,
`### Changed`, and `### Fixed` headings in that order, omitting empty ones,
and SHALL put an ungrouped fragment under `### Changed`.

#### Scenario: No group

- **WHEN** a section's fragments name no group
- **THEN** the section is `## demo 1.0.1` followed by one bullet per fragment

#### Scenario: Ungrouped fragment among grouped ones

- **WHEN** a section has a `fixed` fragment, an `added` fragment, and one with no group
- **THEN** the section lists the added entry under `### Added`, the ungrouped entry under `### Changed`, and the fixed entry under `### Fixed`
