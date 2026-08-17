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

Render through minijinja (Jinja2) files. Defaults live in this crate
(`templates/changelog.jinja`, `templates/dependency-changelog.jinja`).
Repos may override the paths. The adapter does not concatenate Markdown
in Rust beyond a typed context.

`internalAuthors` is adapter policy, not a template `if`. Match the
GitHub login GitHub actually resolves from the commit author. Those
logins omit the byline.

Context: `summary`, `pull_request`, `commit`, `continuations`,
`summary_has_terminal`, `dependencies`.

## Version PR

Happy path is GitHub Actions: `victor-software-house/verctl/actions/version-pr`,
not `changesets/action`, not a GitHub App. The workflow already has
`git`, `gh`, and `${{ github.token }}`. No PAT. No App install.

`prepare` writes version files only (local / recovery).
`prepare --pr` writes CHANGELOG, deletes consumed fragments, and opens
or force-updates `version-packages` through `git2` + `octocrab`.
Auth is `GITHUB_TOKEN` / `GH_TOKEN` only. Push uses that token over
HTTPS, not the machine git/ssh account. We do not call `git` or `gh`
as commands. Local `--pr` is recovery when that same token is already
in the environment.

`prepare --dry-run` and `prepare --preview` print the same plan and
write nothing. `prepare --pr --dry-run` also lists consumed fragments
and whether the Version PR would open or update. Preview does not
require GitHub auth.

`publish` ships the versions already on HEAD: `cargo publish --locked`
for Cargo.toml packages, `npm publish` for package.json, then a GitHub
Release `v{version}` via octocrab. Auth is `GITHUB_TOKEN` plus
`CARGO_REGISTRY_TOKEN` / `NPM_TOKEN`. Already-published crates are
skipped. `--dry-run` / `--preview` print the plan and write nothing.

Happy path after the Version PR merges is
`victor-software-house/verctl/actions/publish`. OIDC trusted
publishing is later (VER-007).

## Stop conditions

Stop and ask when a fragment is not valid YAML, when a package name is
unknown to `[release]` config, or when a `major` fragment lands on a
0.x package.
