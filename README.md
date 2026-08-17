# verctl

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/banner-dark.svg">
  <img src="docs/banner.svg" alt="verctl — Version PRs from Changesets-format fragments. Any stack.">
</picture>

It reads `.changeset/*.md`, bumps declared version files, writes
changelogs, and opens or updates one prepare-release PR.

It is not `@changesets/cli`, not Knope, and not a forkctl verb.

```sh
mise run ver -- status
mise run ver -- check
mise run ver -- prepare
mise run ver -- prepare --pr
mise run ver -- publish
```

## Setup

```toml
min_version = "2026.7.7"

[settings]
experimental = true
lockfile = true

[task_config]
includes = [
  "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=<tag>",
]
```

```sh
mise run ver -- status
mise run ver -- check
mise run ver -- publish --dry-run
```

Until a GitHub Release exists, build from this repo:

```sh
cargo run -- status
```

## Fragments

Same file shape as Changesets. YAML fence. Quoted or unquoted keys.

```md
---
forkctl: patch
"@scope/pkg": minor
---

Restore mise.toml when later patches unapply.
```

Allowed bumps: `major`, `minor`, `patch`, `none`.

## Changelog

Rendered with [minijinja](https://crates.io/crates/minijinja) 2.x from
these defaults (override either path):

- [`templates/changelog.jinja`](templates/changelog.jinja)
- [`templates/dependency-changelog.jinja`](templates/dependency-changelog.jinja)

Default release line:

```text
Internal + PR     - Summary ([#12](https://github.com/org/repo/pull/12)).
External + PR     - Summary ([#12](https://github.com/org/repo/pull/12) by [@ext](https://github.com/ext)).
Commit only       - Summary ([`96fb0bc`](https://github.com/org/repo/commit/96fb0bc)).
No link           - Summary.
```

No extra thanks line. If a PR exists, the SHA is omitted.

Author filtering is not a template `if`. Config `internalAuthors` is
matched against the GitHub login for the commit. Those logins get no
byline. Everyone else does.

## Version files

`verctl prepare` applies fragment bumps through **drivers**.
Local writes are the default. `prepare --pr` is the Version PR
(Action happy path; the token is `GITHUB_TOKEN`, not the `gh` account).
`prepare --dry-run` / `--preview` prints the plan and writes nothing.
`prepare --pr --preview` also shows consumed fragments and open vs update.
Cargo and npm are stock drivers, not a separate code path.

```toml
[drivers.cargo]
format = "toml"
keys = ["workspace.package.version", "package.version"]
# after is optional. If omitted, verctl looks at lockfiles /
# packageManager (bun, pnpm, yarn, npm) and Cargo.lock.

[drivers.npm]
format = "json"
keys = ["version"]
# after = "mise run install"          # overrides detection

[[packages]]
name = "verctl"
path = "Cargo.toml"
# driver = "cargo"   # inferred from the file name

[[packages]]
name = "other"
path = "VERSION"
read = "ver-read-version"          # mise run ver-read-version
write = ["printenv", "VERCTL_VERSION"]
```

A string is a **mise task**. An array is execvp (no shell).
Stdin is the file. Write drivers also get `VERCTL_VERSION`.
Stdout is the version (read) or the new file (write).
`after` is printed, not run.

`0.x` rejects a `major` fragment.

## Runners

Which machines run the work is configuration. It is declared on the thing
that runs, in two places, with the same two fields in both.

| section | one row is | `id` is | `runs_on` defaults to |
|:--|:--|:--|:--|
| `[[ci.jobs]]` | a PR / push validation job | the job name | `["ubuntu-latest"]` |
| `[[assets.targets]]` | a release build job and its tarball | the job name, the `--build` argument, part of the filename | the built-in record for `id` |

```toml
# Omit for one `verify` job on ubuntu-latest, which is the default.
[[ci.jobs]]
id = "verify"
runs_on = ["nscloud-ubuntu-24.04-amd64-4x8"]

# Omit for a library. Both spellings below are the same TOML.
[[assets.targets]]
id = "darwin-arm64"

[[assets.targets]]
id = "linux-x64"
runs_on = ["self-hosted", "linux", "x64"]
```

`runs_on` is the literal GitHub label list — verctl resolves nothing and
knows no aliases, so what is written here is what `runs-on:` receives, and a
label no machine carries queues the way any wrong label queues. A list of
several labels means AND, as GitHub reads it. A row is always a table:
`targets = ["linux-x64"]` is rejected, `targets = [{ id = "linux-x64" }]` is
the same document as the block form above.

Built-in target records, so a public repo needs no config to stay on free
hosted minutes:

| `id` | `runs_on` | `os` | `arch` | `triple` |
|:--|:--|:--|:--|:--|
| `darwin-arm64` | `["macos-latest"]` | `darwin` | `arm64` | `aarch64-apple-darwin` |
| `linux-x64` | `["ubuntu-latest"]` | `linux` | `x64` | `x86_64-unknown-linux-gnu` |

An `id` outside that set describes a platform verctl knows nothing about, so
it must describe all of it — `runs_on`, `os`, `arch`, and `triple` are all
required, and a partial record is an error rather than a build against an
empty target triple. `os = "darwin"` renders as `macos` in the filename;
that is the only rename.

`verctl ci` and `verctl assets` print what resolved, so a default is visible
output rather than an assumption. Both also write a matrix for
`$GITHUB_OUTPUT`: GitHub needs `runs-on` before a job exists, so a small
`plan` job emits the labels and the real jobs consume them
([`examples/workflows/ci.yml`](examples/workflows/ci.yml)).

## Actions

These replace `changesets/action`. They are **GitHub Actions, not a
GitHub App**. The runner already has `git` and `GITHUB_TOKEN`. PRs are
opened with octocrab. Commits and push are `git2` with that token,
not the `git`/`gh` CLIs. There is no local token setup.

| Action | Role |
|:--|:--|
| [`actions/version-pr`](actions/version-pr/action.yml) | Open or update the Version PR |
| [`actions/publish`](actions/publish/action.yml) | Run after that PR merges |

Example consumer workflow: [`examples/workflows/version-pr.yml`](examples/workflows/version-pr.yml).

Mise owns tools. Publish uses Actions OIDC, not a PAT.

## License

MIT.
