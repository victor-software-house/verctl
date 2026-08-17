# verctl

Stack-agnostic Version PR CLI. Fragments are Changesets YAML.
Changelogs are minijinja (Jinja2) with `internalAuthors` filtering.
Ship `actions/version-pr` and `actions/publish`. `changelog.ts` is not
used.

Operator contract: `verctl instructions` and `skills/verctl/SKILL.md`.
This repo's queue is [`tasks.yaml`](tasks.yaml) (`VER-###`).

## Strings (do not regress)

Multiline Rust is `indoc!` / `formatdoc!` / `writedoc!` / `printdoc!` /
`eprintdoc!` / `concatdoc!`. **No `concat!`.** **No escaped `\n` in a
document.** A changelog, fragment, TOML, JSON, or any other fixture
that is more than one line is an `indoc!` block.

```rust
// yes
indoc! {"
    # Changelog

    ## demo 1.0.1

    - Patch.
"}

// no
"## demo 1.0.1\n\n- Patch.\n"
```

Leave a raw `\n` only when that *is* the test: a single `'\n'`
character, CRLF (`\r\n`) fixtures, or a one-line payload such as
`"1.2.3\n"` / `tr -d '\n'`.

## Pretty and color

ctl-core owns tables. `grid` / `kv` take `ColorMode`. Tests pass
`--color never` or call `grid(ColorMode::Never, …)`. No homemade
`strip_ansi`. Packages print as a **grid** (`name` / `version` / `via`),
not a kv dump.

## Argv fixtures

One token per line. Flag+value stay together (`"--cwd", "packages/pkg"`).
Do not glue `"bun", "publish", "--tolerate-republish"` onto one line.

## Process and mise

`process::run_inherit` for cargo / bun / rustup. No `mise exec --`.
mise tasks only.

## Release loop

Human writes `.changeset/*.md` only. Never hand-edit versions or
CHANGELOG. `prepare --pr` is the Version PR. `verctl check --versions`
compares each manifest to the merge-base of HEAD and the default
branch. Only `version-packages` is exempt. CI does not skip.
The actions run `verctl` from PATH. Workflows install the released
tarball. `mise run ver` is only for developing this checkout.
`publish` is exact-SHA plus a matching per-package CHANGELOG heading.
Cargo and bun are stock recipes, not the architecture. Do not name
private release skills in this repo.

## Git

Conventional commits. lefthook. No `--no-verify`. Branch
`type/number-desc`. Squash title = PR title, body = PR description.
