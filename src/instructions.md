# verctl agent instructions

`verctl` consumes Changesets-format fragments and prepares a Version PR.
It is not `@changesets/cli` and not a forkctl verb.

## Invocation

```
mise run ver -- status
mise run ver -- instructions
```

Until a tagged GitHub Release exists, use a local or
`cargo install --git` build, then `verctl`.

## Fragments

`.changeset/<slug>.md`. YAML fence. Quoted or unquoted keys. Values are
`major` | `minor` | `patch` | `none`. Body is the changelog summary.

```md
---
forkctl: patch
"@scope/pkg": minor
---

What changed.
```

Do not invent a third fragment format. Do not infer bumps from
`feat:` / `fix:` unless the repo has turned that mode on (not first slice).

## Changelog

Render through Liquid templates. Defaults live in this crate
(`templates/changelog.liquid`, `templates/dependency-changelog.liquid`).
Repos may override the paths. The adapter does not concatenate Markdown
in Rust beyond passing a typed context into Liquid.

Context matches greenfield-release / pi-stuff: `release.summary`,
`release.pullRequest`, `release.commit`, `release.continuations`,
`release.summaryHasTerminal`, `internalAuthors`, `dependencies`.

## Version PR

`prepare` consumes fragments, writes declared version files and
CHANGELOG sections, opens or updates one PR. Nobody hand-edits
`Cargo.toml` version or CHANGELOG on the happy path.

## Stop conditions

Stop and ask when a fragment is not valid YAML, when a package name is
unknown to `[release]` config, or when a `major` fragment lands on a
0.x package.
