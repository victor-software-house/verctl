## ADDED Requirements

### Requirement: A package owns the files under the version source

A package SHALL own the directory of its version source and every file below it that no deeper package owns.
`files` SHALL replace that set with globs.

#### Scenario: A nested package

- **WHEN** packages live at `.` and `crates/core`, and `crates/core/src/lib.rs` changed
- **THEN** the change belongs to `crates/core` and not to the root package

### Requirement: check reports a changed package with no fragment

`check` SHALL report each package whose owned files changed since the merge-base with the default branch while no fragment names it.
A changed file no package owns SHALL be reported once.

#### Scenario: A change with no fragment

- **WHEN** `packages/a/index.ts` changed on the branch and no fragment names `a`
- **THEN** `verctl check` exits non-zero and names `a`
