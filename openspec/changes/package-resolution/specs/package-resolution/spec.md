## ADDED Requirements

### Requirement: A directory path resolves to the version source it holds

When a package's `path` names a directory, verctl SHALL use the one manifest in it from `Cargo.toml`, `package.json`, `pyproject.toml`, and `go.mod`, and `<dir>/VERSION` when it holds none.
A directory that holds more than one of those manifests SHALL fail the load, naming each one and asking for a file path.

#### Scenario: A repository with no manifest

- **WHEN** `.ctl/ver.yaml` declares `{name: templates, path: .}` and the root holds `VERSION` and no manifest
- **THEN** verctl reads the version of `templates` from `VERSION`

#### Scenario: A directory with two manifests fails

- **WHEN** a package has `path: tools/gen` and `tools/gen` holds both `Cargo.toml` and `package.json`
- **THEN** `verctl check` exits non-zero and its error names both files

### Requirement: Packages with no registry release on GitHub only

A package SHALL release as `publisher: none` does, with a tag and a Release and no registry upload, when both of these hold:

- its version source is a `VERSION` file, a marker line, or `go.mod`;
- it declares no `publisher`, `publish`, or `publishers` entry under its name.
A `pyproject.toml` package with no `publisher`, `publish`, or `publishers` entry under its name SHALL fail the load and name the `publisher` key, because verctl has no stock Python publisher yet.

#### Scenario: A VERSION package releases on GitHub

- **WHEN** package `templates` has `path: VERSION`, no `publisher`, and its Version PR has merged
- **THEN** `verctl publish --plan` lists the tag and the Release for `templates` and no upload

#### Scenario: A Python package must name its publisher

- **WHEN** a package has `path: pyproject.toml`, no `publisher`, no `publish`, and no `publishers` entry under its name
- **THEN** every command that loads `.ctl/ver.yaml` exits non-zero and its error names `publisher`

### Requirement: Two packages never share a changelog

verctl SHALL fail the load when two packages resolve to the same changelog file, naming both packages and the `changelog` key that separates them.

#### Scenario: Two workflows in one directory

- **WHEN** packages `ci` and `release` have `marker: true` paths in `.github/workflows/` and neither sets `changelog`
- **THEN** `verctl check` exits non-zero and its error names `ci`, `release`, and `changelog`
