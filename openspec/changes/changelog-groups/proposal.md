# Group changelog entries under headings

## Why

A release with many entries reads as one flat list, so a reader cannot
pick out what was added from what was fixed.
The operator chose an optional fragment key over a new config file:
old fragments stay valid and old changelogs render the same.

Queue row: VER-052.
Lands on `next` for 0.4.0: `Fragment` gains a public field, and
`cargo semver-checks` 0.51.0 reports `constructible_struct_adds_field`
against the 0.3.16 release, which needs a breaking bump.

## What changes

1. A fragment may set `verctl:group` to `added`, `changed`, or `fixed`.
   The colon cannot appear in a Cargo or npm package name, so the key
   never shadows a package.
2. A section where no fragment names a group renders as today, a flat list.
3. Once one fragment in a section names a group, the section gets
   `### Added`, `### Changed`, and `### Fixed` headings in that order, and an
   ungrouped fragment goes under `### Changed`.
4. An unknown group value fails the parse and names the file.

## Impact

1. `src/fragment.rs`, `src/release.rs`, tests, operator docs, README.
2. `Fragment` has a new public field, `group`.
