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
for Cargo.toml, `bun publish --tolerate-republish` for package.json
(never `npm publish`), then a GitHub Release `v{version}`. GitHub
Packages is `registry = "github"` (`--registry npm.pkg.github.com`);
auth is `bunfig.toml` + `GITHUB_TOKEN`, not `.npmrc`. Cargo already
on crates.io is treated as success. `--dry-run` prints the plan.

Happy path after the Version PR merges is
`victor-software-house/verctl/actions/publish`. It runs when the
`version-packages` PR is merged. Native tarballs are a second job,
only when `[assets].targets` is non-empty. PR CI stays on one
runner. `verctl assets` prints the matrix; `--build` + `--upload`
is `actions/asset`. OIDC trusted publishing is later (VER-007).

## Stop conditions

Stop and ask when a fragment is not valid YAML, when a package name is
unknown to `[release]` config, or when a `major` fragment lands on a
0.x package.
