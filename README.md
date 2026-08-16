# verctl

Stack-agnostic **Version PR** tool. Consumes Changesets-format
`.changeset/*.md` fragments (quoted npm names and unquoted crate names).
Opens or updates a prepare-release PR. Writes versions and changelogs.
Not `@changesets/cli`. Not Knope. Not inside forkctl.

```sh
mise run ver -- add
mise run ver -- status
mise run ver -- prepare
```

## Stack

Rust, same 1.97 / clap / `mise github:` shape as forkctl and qctl. The
binary is the changelog adapter (`changelog.ts` is Node glue; this crate
replaces it). Templates are **Go `text/template`** (chezmoi-style). Defaults:

| File | Job |
|:--|:--|
| `templates/changelog.tmpl` | Release bullets, PR preferred, commit fallback, external-author byline, continuations |
| `templates/dependency-changelog.tmpl` | `- Updated dependencies:` list |

**Author filtering is adapter policy, not a template `if`.** Config
`internalAuthors` is matched against the GitHub login resolved from the
commit. Those logins get no byline. Everyone else does.

Consumers may override the two `.tmpl` paths. Default output:

| Case | Output |
|:--|:--|
| Internal work, PR exists | `- Summary ([#PR](url)).` |
| External work, PR exists | `- Summary ([#PR](url) by [@external](profile)).` |
| No PR, commit known | ``- Summary ([`96fb0bc`](commit-url)).`` |
| No commit | `- Summary.` |

No `Thanks` line. PR over SHA, never both.

Batteries: Cargo.toml / Cargo.lock, package.json + a lockfile task.
Generic regex writer later. Conventional Commits later (synthesizes the
same `.md` files).

Shipped Actions (not a GitHub App):

| Action | Job |
|:--|:--|
| `victor-software-house/verctl/actions/version-pr` | Version PR (`changesets/action` `version:` replacement) |
| `victor-software-house/verctl/actions/publish` | Post-merge publish hook (backends land with VER-007) |

See `examples/workflows/version-pr.yml`. Mise owns tools. OIDC stays on Actions.

Do not name this `changesets` or `knope`.
