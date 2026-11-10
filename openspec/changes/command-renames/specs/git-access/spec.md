## MODIFIED Requirements

### Requirement: Hook variables do not redirect verctl

verctl SHALL read the repository that covers its directory, as it is on disk, and SHALL ignore `GIT_INDEX_FILE`, `GIT_WORK_TREE`, and `GIT_DIR`.
verctl SHALL NOT write the index.

#### Scenario: Prepare inside a pre-commit hook

- **WHEN** `verctl version --dry-run` runs with `GIT_INDEX_FILE=.git/hook-index` naming an empty index, `GIT_DIR=.git`, and `GIT_WORK_TREE=.`
- **THEN** it reports the same plan as without those variables, `.git/index` keeps its bytes, and `git status --porcelain` prints nothing
