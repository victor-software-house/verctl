## ADDED Requirements

### Requirement: A `patch!` entry renders under Breaking

In the section of a package that a fragment bumps by `patch!`, the fragment's entry SHALL render under a `### Breaking` heading, first, whatever its `verctl:group`.
The other entries of that section SHALL render under their groups.
In the section of another package the same fragment bumps, its entry SHALL render as an entry without the mark.

#### Scenario: A breaking patch beside a fix

- **WHEN** the `demo 0.8.1` section has a `demo: patch!` fragment and a `demo: patch` fragment with no group
- **THEN** the section lists the first entry under `### Breaking` and the second under `### Changed`, in that order

#### Scenario: The other package of the fragment

- **WHEN** a fragment sets `demo-core: patch!` and `demo-cli: patch`, and no other fragment names a group
- **THEN** the `demo-core` section lists the entry under `### Breaking`
- **AND** the `demo-cli` section is a flat list
