---
name: verctl
description: >-
  Operate verctl version PRs from Changesets-format fragments. Use when the
  user mentions verctl, mise run ver, Version Packages, prepare-release,
  .changeset fragments, or stack-agnostic changelog templates. Not
  @changesets/cli and not a forkctl verb.
license: MIT
---

# verctl

`verctl instructions` is the installed contract. `mise run ver --` is
for developing this repo (`cargo run`). The actions run `verctl` from
PATH; workflows install the released GitHub tarball.

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted
keys). Changelog Markdown is rendered with minijinja. Author
filtering is `internalAuthors` in config, not template branches. Consumers use
`victor-software-house/verctl/actions/version-pr`, not changesets/action.
Do not assemble changelog strings in ad-hoc Rust. Do not add a Node adapter.

Stop when a 0.x package gets `major`, or when a fragment package is not
in `[release]` config.
