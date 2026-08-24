# verctl

Stack-agnostic Version PR CLI. Fragments are Changesets YAML.
Changelogs are minijinja (Jinja2) with `internalAuthors` filtering.
Ship `actions/version-pr`, `actions/publish`, and `actions/asset`.
`changelog.ts` is not used.

Operator contract: `verctl instructions` (installed with the binary) and
`skills/verctl/SKILL.md` (operator skill, served from
`.ctl/templates/SKILL.md.jinja`). This repo's queue is
[`tasks.yaml`](tasks.yaml) (`VER-###`). Do not put operator invocation in
this file. There is no repo-local `.agents/skills/`.

The `ver` task's `#USAGE mount` line is `ctl_core::mount_line("ver")`.
Put it in `.ctl/templates/ver.jinja`. Do not copy it onto `tasks/ver/ver`
while that file still pins a release that lacks `--usage-spec`.

Operator templates render a ctl-core `Surface` from `Cli`. The shared command
inventory is the only verb list, and the shared invocation fragment owns the
mounted examples and no-`--` rule. `operator_docs` byte-compares both renders
with the committed skill and installed instructions; update them with
`UPDATE_OPERATOR_DOCS=1 cargo test operator_docs`.

## Strings (do not regress)

Multiline Rust is `indoc!` / `formatdoc!` / `writedoc!` / `printdoc!` /
`eprintdoc!` / `concatdoc!`. **No `concat!`.** **No escaped `\n` in a
document.** A changelog, fragment, YAML, TOML, JSON, or any other fixture
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

## Declared input is a schema

A repo writes its declarations in **`.ctl/ver.yaml`** and nowhere else, and its
served-file templates in `.ctl/templates/`. `.ctl/` is shared with the other ctl
CLIs — qctl reads `.ctl/q.yaml`, forkctl `.ctl/fork.yaml` — so nothing under it
is verctl's by virtue of sitting there. `verctl.toml` is gone: not read, not
tolerated. Every `path` in the file is relative to the directory holding `.ctl/`,
which `config::root_of` derives.

A template in that tree must be tracked, and one the repo neither tracks nor
ignores fails the run — rendering nothing would leave the stale file served, and
that is the failure served files exist to end. Ignored is disowned, a symlink is
not followed, and outside the source tree was never ours to render. A served
mise task execs its tool from PATH: `mise where` reads the surrounding config,
so it ignores the task's own `#MISE tools` pin.

Anything a repo or a template declares is **one struct**, parsed once at the
boundary and validated there — never a series of lookups with conversions at use
sites.

- `serde` parses shape and types. `garde` validates the parsed value
  (`#[garde(custom(…))]`), so a complaint names its field. Every schema is
  validated once, at its boundary — `Config::parse` for the file a repo writes,
  `Declared::parse` for what a template exports — and nothing downstream
  re-checks. A rule that needs the rest of the document takes it as garde's
  context (`#[garde(context(Config as config))]`).
- Rules live in `src/schema.rs` when more than one schema wants them, and they
  say what a person has to change: "must declare at least one label", not
  "length is lower than 1". Never ship a stock validator's wording.
- Every field states what it means when unsaid, in its doc comment, and gets its
  default from `#[serde(default)]`. A default that cannot be written as an
  attribute (one derived from a file name) is seeded before parsing, so the field
  stays non-optional.
- `Option` means a real third state, not "unset". If a file is either executable
  or not, the field is `bool`.
- A thing several places share is **named once and listed by name** where it
  applies (`patterns.install`, then `patterns: [install]`), never repeated
  and never bound by where an entry happens to sit. Names resolve once at the
  boundary; an unresolved name, a name listed twice, and a declaration nothing
  lists all fail the load.
- Vocabulary that a bare number cannot express gets an enum with named variants
  (`Occurrences::{Once, Many, Never, Exactly, AtLeast}`), never a magic value.
- Facts about other tools are checked against their documentation, and the
  reference goes in the doc comment. Git records only `0644`/`0755` for a regular
  file, so `executable` is the only mode a template may declare.

## Comments

Doc comments carry the why, on the item. Inline `//` prose inside a function
body is litter — if a line needs explaining, name it better or lift it into the
item's doc comment.

## Presentation

Command handlers return one serializable `Report`; they never print, select a
view, or inspect the terminal. `presentation.rs` maps that model to ctl-core
`Document`, `Fields`, and `Table` primitives through `Present`. ctl-core alone
owns streams, color, width, wrapping, and table rendering. Tests use an explicit
width and `ColorMode::Never` for stable snapshots. JSON serializes the same
report and never contains ANSI. Packages remain a table (`name` / `version` /
`via`), not fields.

## Argv fixtures

One token per line. Flag+value stay together (`"--cwd", "packages/pkg"`).
Do not glue `"bun", "publish", "--tolerate-republish"` onto one line.

## Process and mise

`process::run_inherit` for cargo / bun / rustup. No `mise exec --`.
mise tasks only.

## Release loop

Human writes `.changeset/*.md` only. **Write the fragment on the same
PR that ships the behavior.** Do not save them up for a release dump.
Never hand-edit versions or
CHANGELOG. `prepare --pr` is the Version PR. `verctl check --versions`
compares each manifest to the merge-base of HEAD and the default
branch. Exempt on `version-packages` locally, or when the GitHub
event has the Version PR label (`verctl:version`). CI does not skip.
mise: `mise.toml` is shared settings only. `mise.dev.toml` is rust
and `cargo run`. `mise.release.toml` is the published tarball.
`.miserc.toml` defaults local `MISE_ENV` to `dev`. `prepare`
rewrites `pins` onto the Version PR commit, because the tag names
that commit and publish pushes nothing but the tag. This repo's own
bootstrap pin in `mise.release.toml` is deliberately not a `pins`
entry: it must name a tarball that already exists. Actions run
`verctl` from PATH.
`publish` is exact-SHA plus a matching per-package CHANGELOG heading.
Cargo and bun are stock recipes, not the architecture. Do not name
private release skills in this repo.

## Git

Conventional commits. lefthook. No `--no-verify`. Branch
`type/number-desc`. Squash title = PR title, body = PR description.
