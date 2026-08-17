---
verctl: minor
---

Runners are configuration. A machine is declared once as `[runners.NAME]` with
`labels`, the literal GitHub label list — every label at once, so one entry is
one machine however many labels it carries. Jobs name machines: `[ci.NAME]`
takes `runners = [...]` and gets one check per machine, `[assets.NAME]` takes a
single `runner` because one tarball is one machine. A name resolves against
`[runners]` or fails listing what is declared; only labels reach `runs-on:`, so
verctl never invents a label and a label can never pass as a machine name.
Omitting `[ci]` runs one `verify` job on `ubuntu-latest`, so a repo that says
nothing behaves as before. New `verctl ci` writes the matrix a `plan` job hands
to `runs-on:`.

`verctl instructions` now states which jobs take a runner and why: verctl
decides how many jobs there are, the workflow file decides where a fixed job
runs. `[ci]` and `[assets]` exist because only the repo knows how many verify
checks or tarballs it wants. `plan`, `crate`, `pin`, and `prepare` are always
one job each, so their `runs-on` stays a literal in the workflow you own. There
is no `[release]` runner table and none is planned.

`$GITHUB_OUTPUT` is appended to rather than truncated, so a plan step no
longer drops assignments an earlier step in the same step file made.

Breaking for anyone who wrote an asset target. `[assets] targets = [...]` is
retired for one `[assets.NAME]` table per target, and naming it is an error
that states the migration:

```toml
# before
[assets]
targets = ["linux-x64"]

# after — built-in name, so nothing else is needed
[assets.linux-x64]

# after — with a machine of your own
[runners.big]
labels = ["self-hosted", "linux", "x64"]

[assets.linux-x64]
runner = "big"
```

An unknown target name with a partial record now fails instead of building
against an empty target triple. `{runner}` is no longer expanded in `[assets]`
`prepare` / `build` / `binary` and has no replacement, since a label list has
no single rendering. A workflow copied from `examples/workflows/publish.yml`
renames `matrix.runner` to `matrix.labels` in the same step as the bump.
