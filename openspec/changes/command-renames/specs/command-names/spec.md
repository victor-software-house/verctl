## ADDED Requirements

### Requirement: The version step is the version command

`verctl version` SHALL apply fragment bumps and take every flag and
variable the version step took in verctl 0.7. Its report SHALL list
follow-up commands under `after`.

#### Scenario: A dry run in JSON

- **WHEN** `.changeset/fix.md` names `demo: patch` and `verctl version --dry-run --format json` runs
- **THEN** the report has an `after` array and no `next` field

### Requirement: The render command writes pins and served files

`verctl render` SHALL rewrite each `pins` entry, then render every template
in `.ctl/templates/`, at the versions on HEAD. It SHALL take every flag the
pins rewrite took in verctl 0.7.

#### Scenario: A dry run

- **WHEN** `.ctl/ver.yaml` declares a `pins` entry for `mise.release.toml` and `verctl render --dry-run` runs
- **THEN** it lists `mise.release.toml` and writes nothing

#### Scenario: A served file

- **WHEN** `.ctl/templates/ver.jinja` renders `tasks/ver/ver` and `verctl render` runs
- **THEN** `tasks/ver/ver` holds the template's output at the versions on HEAD

### Requirement: The release command releases HEAD

`verctl release` SHALL release the versions on HEAD and take every flag and
variable the release step took in verctl 0.7, except `--plan`.

#### Scenario: A dry run

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml` with `publisher: none` and `verctl release --dry-run` runs
- **THEN** it lists `demo` via `release only` and would create `v1.0.0`

### Requirement: The release asset command builds one tarball

`verctl release asset <target>` SHALL build the tarball of one `assets`
target, read the target from `VERCTL_TARGET` when it is not given, and
upload the tarball unless `--dry-run` is set. With `--vendored <name>` it
SHALL build and upload that set's archive instead.

#### Scenario: A dry run

- **WHEN** `.ctl/ver.yaml` declares the `linux-x64` target and `VERCTL_TARGET=linux-x64 verctl release asset --dry-run` runs
- **THEN** it builds the tarball and uploads nothing

### Requirement: status lists unreleased version commits

`verctl status` SHALL list each version commit whose release is incomplete
under `unreleased`, oldest first, with the asset plan beside the
fragments. Without `GITHUB_TOKEN` or `GH_TOKEN`, or when a lookup cannot
run and `--github-output` is not set, it SHALL report `unreleased` as
`null` in JSON with the reason in `unreleased_unchecked`. Any other error
SHALL fail `status`.

#### Scenario: No token

- **WHEN** neither `GITHUB_TOKEN` nor `GH_TOKEN` is set and `verctl status --format json` runs
- **THEN** it exits zero, `unreleased` is `null`, and `unreleased_unchecked` is `no GITHUB_TOKEN or GH_TOKEN`

#### Scenario: A lookup that cannot run

- **WHEN** `GITHUB_TOKEN` is set, the clone has no GitHub `origin`, and `verctl status --format json` runs
- **THEN** it exits zero, `unreleased` is `null`, and `unreleased_unchecked` holds the error
- **AND** `verctl status --github-output out` in the same clone exits non-zero

#### Scenario: A release tag on the wrong commit

- **WHEN** `GITHUB_TOKEN` is set and GitHub reports that tag `v1.0.0` names a commit HEAD does not hold at `demo@1.0.0`, and `verctl status` runs
- **THEN** it exits non-zero and the error names the tag

### Requirement: status writes the release plan

`verctl status --github-output <file>` SHALL fetch the history the plan
needs and write `has_assets`, `tag`, `matrix`, `commits`, `pending`, and
`assets` to the file. Without `--github-output`, `status` SHALL NOT fetch
or unshallow the clone.

#### Scenario: A package at 0.0.0

- **WHEN** `.ctl/ver.yaml` declares only `demo` at `0.0.0` and `verctl status --github-output out` runs with a token
- **THEN** `out` holds `pending=false` and `commits=[]`

### Requirement: A removed verb names its replacement

`verctl prepare`, `pin`, `publish`, and `assets` SHALL exit 2 without
reading the config, and their error SHALL name the command that replaced
them. None SHALL appear in `--help`. The usage spec SHALL mark each
`hide`, which keeps them out of mise help and completion. `verctl release
--plan` SHALL fail the parse and name `verctl status --github-output`.

#### Scenario: The old version verb

- **WHEN** a user runs `verctl prepare --pr`
- **THEN** it exits 2 and stderr says `prepare is now version: run verctl version`

#### Scenario: The old pin verb under JSON

- **WHEN** a user runs `verctl --format json pin --dry-run`
- **THEN** it exits 2 and the JSON error on stdout says `pin is now render: run verctl render`

#### Scenario: The old assets verb

- **WHEN** a user runs `verctl assets --build linux-x64 --upload`
- **THEN** it exits 2 and stderr says `assets is now release asset: run verctl release asset`

#### Scenario: The old plan flag

- **WHEN** a user runs `verctl release --plan`
- **THEN** it exits 2 and stderr names `verctl status --github-output`

#### Scenario: The usage spec

- **WHEN** mise runs `verctl --usage-spec=ver` to read the task grammar
- **THEN** the `cmd prepare`, `cmd pin`, `cmd publish`, and `cmd assets` nodes carry `hide=#true`
