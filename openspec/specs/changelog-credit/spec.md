# changelog-credit Specification

## Purpose
TBD - created by archiving change changelog-internal-authors. Update Purpose after archive.

## Requirements

### Requirement: Only unlisted people from forks are credited

With a token, verctl SHALL credit an entry's pull request author only when
the author's login is not in `changelog.internal_authors`, the author's
`type` is `User`, and the pull request's head branch lives outside the
repository. Without a token, or when the lookup fails, it SHALL credit no
one.

#### Scenario: Listed maintainer on an in-repo branch

- **WHEN** `.ctl/ver.yaml` lists `any-victor` in `changelog.internal_authors`, and pull request #12 was opened by `any-victor` from a branch in `acme/demo`
- **THEN** the entry reads `([#12](https://github.com/acme/demo/pull/12))` with no byline

#### Scenario: Listed maintainer from a fork

- **WHEN** `.ctl/ver.yaml` lists `any-victor`, and pull request #12 was opened by `any-victor` from the fork `any-victor/demo`
- **THEN** the entry carries no byline

#### Scenario: Outside contributor

- **WHEN** pull request #12 was opened by `octo`, a `User` not in `changelog.internal_authors`, from the fork `octo/demo`
- **THEN** the entry reads `([#12](https://github.com/acme/demo/pull/12) by [@octo](https://github.com/octo))`

#### Scenario: Bot

- **WHEN** pull request #12 was opened by `renovate[bot]`, a `Bot`, from a fork
- **THEN** the entry carries no byline

### Requirement: Config keys are additive

`Config` and every config struct SHALL be `#[non_exhaustive]`.

#### Scenario: A new key

- **WHEN** a later release adds a field to `Config` and `cargo semver-checks` compares it with 0.4.0, which already marks `Config` `#[non_exhaustive]`
- **THEN** the check reports no breaking change for that field
