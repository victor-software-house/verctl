# verctl

Version PRs from Changesets-format fragments. Any stack.

It reads `.changeset/*.md`, bumps declared version files, writes
changelogs, and opens or updates one prepare-release PR.

It is not `@changesets/cli`, not Knope, and not a forkctl verb.

```sh
mise run ver -- add
mise run ver -- status
mise run ver -- prepare
```

Today `status`, `check`, and `instructions` are implemented.
`add` and `prepare` are next.

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

`verctl prepare --no-pr` applies fragment bumps through **drivers**.
Cargo and npm are stock drivers, not a separate code path.

```toml
[drivers.cargo]
format = "toml"
keys = ["workspace.package.version", "package.version"]
# after = "cargo generate-lockfile"   # you choose

[drivers.npm]
format = "json"
keys = ["version"]
# after = "npm install"               # or bun / pnpm / mise run install

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

## Actions

These replace `changesets/action`. They are GitHub Actions, not an App.

| Action | Role |
|:--|:--|
| [`actions/version-pr`](actions/version-pr/action.yml) | Open or update the Version PR |
| [`actions/publish`](actions/publish/action.yml) | Run after that PR merges |

Example consumer workflow: [`examples/workflows/version-pr.yml`](examples/workflows/version-pr.yml).

Mise owns tools. Publish uses Actions OIDC, not a PAT.

## License

MIT.
