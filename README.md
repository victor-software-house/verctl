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
replaces it). Templates stay **Liquid**, the same files greenfield-release
already documents:

| File | Job |
|:--|:--|
| `templates/changelog.liquid` | Release bullets, PR preferred, commit fallback, external-author byline, continuations |
| `templates/dependency-changelog.liquid` | `- Updated dependencies:` list |

Consumers may override those two paths. Defaults match
[greenfield-release](https://github.com/victor-software-house) /
pi-stuff output:

| Case | Output |
|:--|:--|
| Internal work, PR exists | `- Summary ([#PR](url)).` |
| External work, PR exists | `- Summary ([#PR](url) by [@external](profile)).` |
| No PR, commit known | ``- Summary ([`96fb0bc`](commit-url)).`` |
| No commit | `- Summary.` |

No `Thanks` line. PR over SHA, never both.

Batteries: Cargo.toml / Cargo.lock, package.json + a lockfile task.
Generic regex writer later. Conventional Commits later (synthesizes the
same `.md` files). Publish stays GitHub Actions + OIDC.

Do not name this `changesets` or `knope`.
