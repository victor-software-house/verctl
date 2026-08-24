---
name: verctl
description: >-
  Operate verctl version PRs from Changesets-format fragments. Use when the
  user mentions verctl, mise run ver, Version Packages, prepare-release,
  .changeset fragments, or stack-agnostic changelog templates. Not
  @changesets/cli and not a forkctl verb.
license: MIT
version: 0.3.0
---

# verctl

`verctl instructions` is the installed contract. Actions run `verctl` from
PATH.

```sh
mise run ver status
mise run ver check
mise run ver prepare --pr
```

Never `mise run ver --`. The `--` in `#USAGE mount` is mise's completion
bootstrap.

Every command returns one typed report. Use `--format json` for the same data
without ANSI; JSON errors use stdout, while human errors use stderr. `--quiet`
suppresses only successful human output.

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted
keys). Changelog Markdown is rendered with minijinja. Author
filtering is `internalAuthors` in config, not template branches. Consumers use
`victor-software-house/verctl/actions/version-pr`, not changesets/action.
Do not assemble changelog strings in ad-hoc Rust. Do not add a Node adapter.

A repo declares everything in `.ctl/ver.yaml`, the one file verctl reads;
`.ctl/` is shared with the other ctl CLIs and templates live in
`.ctl/templates/`. There is no `verctl.toml`.

Prefer a template over a pin: a served file is generated, and a `pins` entry
admits a file is hand-authored. A template declares its own target with
top-level `{%- set … -%}` exports. Commit it — one the repo neither tracks nor
ignores fails the run, because rendering nothing would serve the stale file
instead. An ignored one is disowned and left alone; one outside the source
tree was never verctl's to render.

A served mise task execs the tool from PATH, never
`"$(mise where <tool>)/<tool>"`. `mise where` resolves from the surrounding
config, so it ignores the task's own `#MISE tools` pin, while mise already puts
that pin first on PATH. Removing it is only safe together with rendering the
pin, or the task freezes on whatever version the file happens to name.

`pin` rewrites those collocated pins onto HEAD.

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

`tags.template` (default `v{version}`) names release tags. `{name}` makes
one tag and one Release per package; without it, differing versions refuse.
Each Release body is that package's CHANGELOG section. The tag is created
at HEAD and re-read; a tag that names another commit fails.
`verctl publish` fetches the default branch (that one ref, with history)
only when `origin/<default>` is missing. It rewrites `origin/HEAD` only
from `VERCTL_DEFAULT_BRANCH`. GitHub git HTTPS uses `x-access-token`,
never Bearer, and never puts the token in the URL.
