# Release from shallow clones and drifted workflow trees

## Why

A consumer adopting the reusable release workflow hit three failures that
verctl either causes or explains badly:

1. `verctl publish` in a depth-1 checkout stops with "cannot find the commit
   that set Cargo.toml to 0.0.2: the clone is shallow; fetch the full
   history" (shrub run 39195633667). verctl already fetches the default
   branch for its ancestry proof, so it can fetch the history it needs
   instead of asking the caller to.
2. A run that published every crate then failed at "create GitHub release:
   GitHub: Resource not accessible by integration" with `contents: write`
   (shrub run 39198937088). GitHub requires `workflows: write` to create a
   Release whose tag names a commit with a `.github/workflows` tree that
   differs from the default branch, and `GITHUB_TOKEN` cannot hold it. The
   error names neither cause nor remedy.
3. Adopting `release.yml` while a Version PR was pending released a version
   commit from before the adoption, which lacks the `ver` task, and failed
   with "no task ver found" (shrub run 39196692461). Finding 2 is the same
   ordering seen from the Releases API.

Queue row: VER-072.
A patch: it changes behaviour only for runs that fail today, and adds no
configuration key.

## What changes

1. With `VERCTL_DEFAULT_BRANCH` set, `verctl publish` and
   `verctl publish --plan` fetch the whole history of `origin/<default>`
   when the clone is shallow, before any history walk. Without it, the
   shallow-clone error stays and names the variable.
2. A 403 from creating a Release fails with a message that names
   `workflows: write`, `GITHUB_TOKEN`'s limit, and the remedy.
3. The operator instructions and README say to adopt `release.yml` only
   when no release is pending, or to merge the adoption before the Version
   PR, and describe the 403.

## Impact

1. `src/git.rs`, `src/publish.rs`, `src/main.rs`, and `src/github.rs`.
2. `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, and
   `README.md`.
3. `.github/workflows/release.yml` is unchanged; see the design.
