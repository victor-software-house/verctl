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

`verctl instructions` is the installed contract. Prefer `mise run ver --`.

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted
keys). Changelog Markdown is rendered from Liquid templates, same output
as greenfield-release / pi-stuff. Do not assemble changelog strings in
ad-hoc Rust. Do not add a Node adapter.

Stop when a 0.x package gets `major`, or when a fragment package is not
in `[release]` config.
