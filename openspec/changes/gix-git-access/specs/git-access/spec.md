## ADDED Requirements

### Requirement: verctl builds without OpenSSL

The dependency graph SHALL contain no `openssl` and no `native-tls` crate.

#### Scenario: Bans hold

- **WHEN** `cargo deny check bans` runs with `deny.toml` banning `openssl` and `native-tls`
- **THEN** it passes

### Requirement: Hook variables do not redirect verctl

verctl SHALL read the repository that covers its directory, as it is on disk, and SHALL ignore `GIT_INDEX_FILE`, `GIT_WORK_TREE`, and `GIT_DIR`.
verctl SHALL NOT write the index.

#### Scenario: Prepare inside a pre-commit hook

- **WHEN** `verctl prepare --dry-run` runs with `GIT_INDEX_FILE=.git/hook-index` naming an empty index, `GIT_DIR=.git`, and `GIT_WORK_TREE=.`
- **THEN** it reports the same plan as without those variables, `.git/index` keeps its bytes, and `git status --porcelain` prints nothing

### Requirement: The push token stays out of argv, URLs, and errors

`git push` SHALL receive the token only through its credential helper's environment.
A failed push SHALL report an error that does not contain the token.

#### Scenario: Rejected token

- **WHEN** the push meets a server that answers 401 to every request
- **THEN** the server received `x-access-token` and the token as Basic credentials, and the error verctl returns does not contain the token
