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

`verctl instructions` is the installed contract. `MISE_ENV=dev` (local
default) compiles this checkout. `MISE_ENV=release` is the published
tarball. Actions run `verctl` from PATH.

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted
keys). Changelog Markdown is rendered with minijinja. Author
filtering is `internalAuthors` in config, not template branches. Consumers use
`victor-software-house/verctl/actions/version-pr`, not changesets/action.
Do not assemble changelog strings in ad-hoc Rust. Do not add a Node adapter.

A repo declares everything in `.ctl/ver.yaml`, the one file verctl reads;
`.ctl/` is shared with the other ctl CLIs and templates live in
`.ctl/templates/`. There is no `verctl.toml`.

Machines are `runners` entries with `labels`, declared once and named by
the jobs that run on them: a `ci` job takes `runners`, one check each; an
`assets` target takes a single `runner`, because one tarball is one
machine. Only `labels` reach `runs-on`; a name resolves against
`runners` or fails.

verctl decides how many jobs there are; the workflow file decides where a
fixed job runs. `plan`, `crate`, and `prepare` are always one job
each, so their `runs-on` is a literal in the consumer's workflow — not a
config key. Do not add one, and do not add a `release` runner section.

Stop when a 0.x package gets `major`, or when a fragment names a package
that is not in `packages`.
