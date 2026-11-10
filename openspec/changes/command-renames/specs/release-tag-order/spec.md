## MODIFIED Requirements

### Requirement: A registry-checked package is tagged before its uploads

`verctl release` SHALL create the tag and GitHub Release before running any
upload of the run. This holds for a tag whose packages all have a registry
check, or no upload.

A run that finds that tag and Release SHALL create neither. That run SHALL
fail before any upload when the tag names a commit other than HEAD.

When GitHub refuses that Release with 403, the run SHALL still run its
uploads and SHALL then fail, naming the refusal.

#### Scenario: The crates.io rate limit stops the first run

- **WHEN** `demo` publishes to crates.io with tag `v0.4.0`, and `cargo publish` fails with status 429 in the run that merged the Version PR
- **THEN** tag `v0.4.0` and its Release exist at the version commit
- **AND** `verctl status` still lists the version commit, because crates.io does not list `demo` `0.4.0`

#### Scenario: A retry after a workflow change

- **WHEN** a pull request that changes `.github/workflows/ci.yml` merges after that run, and `verctl release` reruns at the version commit with `GITHUB_TOKEN`
- **THEN** it uploads `demo` and creates no tag or Release

#### Scenario: A release-only package

- **WHEN** `app` declares `publisher: none`
- **THEN** `verctl release` creates its tag and Release before any upload of the run

### Requirement: An unchecked upload keeps its tag after the upload

`verctl release` SHALL create a tag that names a package with an unchecked
upload only after that upload succeeds. The tag is the record that the upload
ran.

#### Scenario: A shared tag over a checked and an unchecked package

- **WHEN** `tags.template` is `v{version}`, `demo` publishes to crates.io, and `tool` declares `publish: [./upload.sh]` with no `query`
- **THEN** `verctl release` runs both uploads before it creates `v0.4.0`
