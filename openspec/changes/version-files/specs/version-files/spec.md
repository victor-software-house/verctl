## ADDED Requirements

### Requirement: A package's file name picks its stock driver

verctl SHALL pick a stock driver from the file name a package's `path` names, with no `driver` key: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, and `VERSION`.
A `driver` key or inline driver fields SHALL still override the stock driver.

#### Scenario: A Python package needs no driver

- **WHEN** `.ctl/ver.yaml` declares `{name: demo, path: pyproject.toml, publisher: none}` and `pyproject.toml` holds `[project]` with `version = "0.3.1"`
- **AND** `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare --dry-run` plans `demo` from `0.3.1` to `0.3.2`

#### Scenario: A declared driver still wins

- **WHEN** the package for `pyproject.toml` declares `driver: custom`, a `drivers` entry with `read` and `write` commands
- **THEN** verctl reads and writes the version through those commands

### Requirement: pyproject.toml holds the version in project.version

The stock `pyproject.toml` driver SHALL read and write `project.version`, and SHALL keep every other byte of the file.
When `project.dynamic` lists `version`, every command that reads the version SHALL fail, name the package, and say that the version comes from a build backend and that a `VERSION` file or a driver fixes it.

#### Scenario: A Python package bumps

- **WHEN** `pyproject.toml` holds `version = "0.3.1"` under `[project]` and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare` writes `version = "0.3.2"` and leaves the rest of `pyproject.toml` byte for byte

#### Scenario: A dynamic version fails

- **WHEN** `pyproject.toml` holds `dynamic = ["version"]` under `[project]`, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl check` exits non-zero and its error names `demo`, `project.dynamic`, and `VERSION`

### Requirement: The bump names the uv.lock refresh

When a `uv.lock` sits beside a `pyproject.toml` package or above it in the repository, `prepare` SHALL name `uv lock` as the bump's follow-up, as it names `cargo update --workspace` for `Cargo.lock`, because the lockfile records the project's own version.

#### Scenario: A uv project bumps

- **WHEN** the repository root holds `uv.lock` beside `pyproject.toml`, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare` lists `uv lock` under its next steps

### Requirement: go.mod holds the version in a comment on the module line

The stock `go.mod` driver SHALL read and write the version as a `// vX.Y.Z` comment at the end of the `module` line, and SHALL keep the rest of the file.
A `module` line with no such comment SHALL read as `0.0.0`.
verctl SHALL refuse to write that comment when the comment block directly above the `module` line ends with its first paragraph that starts with `Deprecated:`, because Go then reads both as one deprecation notice.

#### Scenario: A Go module bumps

- **WHEN** `go.mod` holds `module example.com/demo // v0.4.0` and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare` writes `module example.com/demo // v0.4.1`

#### Scenario: A module with no version comment starts at 0.0.0

- **WHEN** `go.mod` holds `module example.com/demo` and `.changeset/first.md` holds `demo: patch`
- **THEN** `verctl prepare` writes `module example.com/demo // v0.0.1`

#### Scenario: A deprecation notice directly above the module line

- **WHEN** `go.mod` holds `// Deprecated: use example.com/next.` directly above `module example.com/demo`, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare --dry-run` exits non-zero and its error names `Deprecated:`

### Requirement: A Go module's major version matches its module path

From major version 2, the Go toolchain reads a module only when its path ends in `/v<major>`.
verctl SHALL refuse a bump whose new major version is 2 or more and differs from the `/vN` suffix of the module path, and the refusal SHALL name the module path the release needs.

#### Scenario: A major bump to v2 without the path change

- **WHEN** `go.mod` holds `module example.com/demo // v1.4.0` and `.changeset/break.md` holds `demo: major`
- **THEN** `verctl prepare --dry-run` exits non-zero and its error names `example.com/demo/v2`

#### Scenario: A v2 module bumps within its major

- **WHEN** `go.mod` holds `module example.com/demo/v2 // v2.1.0` and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl prepare --dry-run` plans `demo` from `2.1.0` to `2.1.1`

### Requirement: A VERSION file holds only the version

The stock `VERSION` driver SHALL read a file that holds one version and an optional final newline, and SHALL write the new version followed by one newline.
A `VERSION` file holding anything else SHALL fail the load with the file's path.
A missing `VERSION` file SHALL read as `0.0.0`, `check` SHALL not create it, and `prepare` SHALL create it with the new version.

#### Scenario: A VERSION file bumps

- **WHEN** `VERSION` holds `0.0.75` and a newline, and `.changeset/fix.md` holds `templates: patch`
- **THEN** `verctl prepare` writes `0.0.76` and a newline to `VERSION`

#### Scenario: A missing VERSION file starts at 0.0.0

- **WHEN** package `setup` has `path: actions/setup/VERSION`, the file does not exist, and `.changeset/first.md` holds `setup: patch`
- **THEN** `verctl check` exits zero and leaves the file missing
- **AND** `verctl prepare` creates `actions/setup/VERSION` holding `0.0.1` and a newline

#### Scenario: A VERSION file with extra text fails

- **WHEN** `VERSION` holds `version 1.2.3`
- **THEN** `verctl check` exits non-zero and its error names `VERSION`

### Requirement: A marker line holds the version of a single-file package

A package that declares `marker: true` SHALL keep its version on one line of its file: a comment opener (`#`, `//`, or `<!--`), then `version:`, a space, and the version, with an optional `-->` after it.
verctl SHALL rewrite only the version on that line, and SHALL fail the load when the file holds no such line or more than one.

#### Scenario: A reusable workflow bumps

- **WHEN** package `ci` has `path: .github/workflows/ci.yml` and `marker: true`, the file holds the line `# version: 1.2.0`, and `.changeset/fix.md` holds `ci: patch`
- **THEN** `verctl prepare` rewrites that line to `# version: 1.2.1` and changes no other line

#### Scenario: A file with two marker lines fails

- **WHEN** a `marker: true` package's file holds `# version: 1.2.0` on two lines
- **THEN** `verctl check` exits non-zero and its error names the file and both line numbers

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

A package whose version source is a `VERSION` file, a marker line, or `go.mod`, and that declares no `publisher`, `publish`, or `publishers` entry under its name, SHALL release as `publisher: none` does: a tag and a Release, with no registry upload.
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
