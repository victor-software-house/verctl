# verctl

Stack-agnostic Version PR CLI.  
Fragments are Changesets YAML.  
Changelogs are minijinja (Jinja2); entries link their pull request and credit no maintainer.  
Ship the reusable workflow `.github/workflows/release.yml`, and `actions/version-pr`, `actions/publish`, and `actions/asset` for hand-written workflows.  
This repository releases through `.github/workflows/self-release.yml`.  
It calls the repository's own reusable workflow at the same commit, where `mise run ver` builds the verctl being released.  
`changelog.ts` is not used.

Operator contract: `verctl instructions` (installed with the binary) and `skills/verctl/SKILL.md` (operator skill, served from `.ctl/templates/SKILL.md.jinja`).  
This repo's queue is [`tasks.yaml`](tasks.yaml) (`VER-###`).  
Do not put operator invocation in this file.  
There is no repo-local `.agents/skills/`.

The `ver` task carries `ctl_core::mount_directives("ver")`: `raw`, `raw_args`, and the `#USAGE mount` line, so piped output stays what verctl wrote.  
Put it in `.ctl/templates/ver.jinja`.  
Do not copy it onto `tasks/ver/ver` while that file still pins a release that lacks `--usage-spec`.

Operator templates render a ctl-core `Surface` from `Cli`.  
The shared command inventory is the only verb list, and the shared invocation fragment owns the mounted examples and no-`--` rule.  
`operator_docs` byte-compares both renders with the committed skill and installed instructions; update them with `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs`.

## Strings (do not regress)

Multiline Rust uses `indoc!`, `formatdoc!`, `writedoc!`, `printdoc!`, `eprintdoc!`, or `concatdoc!`.  
**No `concat!`.**  
**No escaped `\n` in a document.**  
A changelog, fragment, YAML, TOML, JSON, or any other fixture that is more than one line is an `indoc!` block.

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

Leave a raw `\n` only when that *is* the test:

- a single `'\n'` character;
- a fixture with carriage return line feed (CRLF) endings, `\r\n`;
- a one-line payload such as `"1.2.3\n"` or `tr -d '\n'`.

## Declared input is a schema

A repo writes its declarations in **`.ctl/ver.yaml`** and nowhere else, and its served-file templates in `.ctl/templates/`.  
`.ctl/` is shared with the other ctl CLIs (qctl reads `.ctl/q.yaml`, forkctl `.ctl/fork.yaml`), so nothing under it is verctl's by virtue of sitting there.  
`verctl.toml` is gone: not read, not tolerated.  
Every `path` in the file is relative to the directory holding `.ctl/`, which `config::root_of` derives.

A template in that tree must be tracked, and one the repo neither tracks nor ignores fails the run.  
Rendering nothing would leave the stale file served, and that is the failure served files exist to end.  
Ignored is disowned, a symlink is not followed, and outside the source tree was never ours to render.

A served mise task execs its tool from `PATH`: `mise where` reads the surrounding config, so it ignores the task's own `#MISE tools` pin.

`check` renders every template and rewrites every pin in memory at the versions the manifests hold, and fails on each file that differs.  
`render` writes the same bytes, so it is the one fix.  
A file that a template and a `pins` entry both claim is refused wherever templates render, before any write.

Anything a repo or a template declares is **one struct**, parsed once at the boundary and validated there.  
Code that uses a value never looks it up and converts it on the spot.

- `serde` parses shape and types.  
  `garde` validates the parsed value (`#[garde(custom(…))]`), so a complaint names its field.  
  Every schema is validated once, at its boundary (`Config::parse` for the file a repo writes, `Declared::parse` for what a template exports) and nothing downstream re-checks.  
  A rule that needs the rest of the document takes it as garde's context (`#[garde(context(Config as config))]`).
- YAML a repo writes (`.ctl/ver.yaml`, fragment frontmatter) is read through ctl-core's `input` feature: `Input` parses it strictly and places each problem on its line.  
  Do not add another YAML parser here.
- Rules shared across schemas come from `ctl_core::validate`.  
  Each message names what a person has to change, such as "must declare at least one label" in place of "length is lower than 1".  
  Never ship a stock validator's wording.
- Every field states what it means when omitted, in its doc comment, and gets its default from `#[serde(default)]`.  
  A default that cannot be written as an attribute (one derived from a file name) is seeded before parsing, so the field stays non-optional.
- `Option` means a real third state, not "unset".  
  If a file is either executable or not, the field is `bool`.
- A thing several places share is **named once and listed by name** where it applies: `patterns.install`, then `patterns: [install]`.  
  It is never repeated, and never bound by where an entry happens to sit.  
  Names resolve once at the boundary; an unresolved name, a name listed twice, and a declaration nothing lists all fail the load.
- Vocabulary that a bare number cannot express gets an enum with named variants (`Occurrences::{Once, Many, Never, Exactly, AtLeast}`), never a magic value.
- Facts about other tools are checked against their documentation, and the reference goes in the doc comment.  
  Git records only `0644`/`0755` for a regular file, so `executable` is the only mode a template may declare.

## Comments

Doc comments carry the why, on the item.  
Inline `//` prose inside a function body is litter: if a line needs explaining, name it better or lift it into the item's doc comment.

## Presentation

Command handlers return one serializable `Report`; they never print, select a view, or inspect the terminal.  
`presentation.rs` maps that model to ctl-core `Document`, `Fields`, and `Table` primitives through `Present`.  
ctl-core alone owns streams, color, width, wrapping, and table rendering.

Tests use an explicit width and `ColorMode::Never` for stable snapshots.  
JSON serializes the same report and never contains ANSI.  
Packages remain a table with the columns `name`, `version`, and `via`, instead of fields.

## Argv fixtures

One token per line.  
Flag+value stay together (`"--cwd", "packages/pkg"`).  
Do not glue `"bun", "publish", "--tolerate-republish"` onto one line.

## Flags and the environment

The root parser carries ctl-core's `env_prefix = "VERCTL"`, so every long option reads `VERCTL_<LONG>`.  
The actions set `env:` and run `exec verctl <verb>`; no action builds argv in bash, and no input restates a verctl default.  
Integration tests start the binary through `tests/common::verctl()`.  
It removes the colour variables and every `VERCTL_*` variable.  
A test that parses `Cli` directly runs `ctl_core::flags::env_under_flags` first, as `App` does.

## Process and mise

Run cargo, bun, and rustup through `process::run_inherit`.  
No `mise exec --`.  
Use mise tasks only.

## Release loop

Human writes `.changeset/*.md` only.  
**Write the fragment on the same PR that ships the behavior.**  
Do not save them up for a release dump.  
Never hand-edit versions or CHANGELOG.  
`version --pr` is the Version PR.

`verctl check --versions` compares each manifest to the merge-base of HEAD and the default branch.  
It exempts the Version PR in two places:

- locally, on a `verctl/release/<base>` branch;
- on GitHub, when the event's pull request comes from this repository's `verctl/release/<base>` branch for its current base, read through the API.

A version the default branch carries at its merge-base with HEAD is not a hand edit.  
On Actions the check fetches the base and default branches when the checkout lacks them or is shallow.  
CI does not skip.

Only the operator merges the Version PR.  
Nothing in CI merges it or enables auto-merge on it, so no release happens on its own.

### Template and tasks

The repository setup comes from the shared Copier template (project `rust-crate`, layout `crate`), and `.copier-answers.yml` records its tag and answers.  
Take a new tag with `copier update --defaults` and check `git diff --check` before committing.  
The `policy` task fails when a workflow names a mise version other than `min_version` in `mise.toml`.  
That includes the `mise-version` default of `release.yml`, which the template does not render.

`template.yml` calls the shared drift check of the public actions repository at the commit the template pins, reported as `template / Template`.  
It fails only a pull request that changes `.copier-answers.yml` without reaching the newest tag.  
A push to `main` or the daily run keeps a "Template update available" issue open while a newer tag would change a file.

One local edit survives an update: the `plan` and matrix `verify` jobs in `ci.yml`.  
There `mise run ver ci` plans the runners, and the `semver` job does not run on a pull request into `next`.  
The shared Rust workflow does not express either.  
`missing_docs`, `missing_debug_implementations`, and the openssl and native-tls bans all apply as the template ships them.

`mise.toml` pins stable 1.99.0 through mr-boxington, whose store CI restores with its action.  
It also pins the dated nightly, for formatting and for `doc`, which builds rustdoc as docs.rs does.  
`rust-toolchain.toml` and `rust-version` match.  
`mise.toml` composes `verify` from the actions repository's `tasks/rust.toml`.

`check:repo` comes from the template and covers TOML format, Markdown, spelling, workflows, and OpenSpec.  
It runs `check:local`, which `mise-tasks/check/local.sh` fills with workflow policy, the version guard, and pending fragments.  
Never edit `check:repo`.  
This repo's other tasks live in `mise-tasks/`.

### CI and the release build

CI adds `deny:advisories`, `semver`, and `cargo package`.  
Its `ci.yml` keeps the `plan` job and the `verify` matrix, so a public repo stays on hosted runners.  
`.rumdl.toml` and `.typos.toml` carry local exceptions.  
`mise.dev.toml` holds only the hook tools and arms the git hooks on `mise install`.  
CI runs `MISE_ENV=ci`.

`self-release.yml` builds verctl once per platform in its `build` job.  
It calls `release.yml` with `verctl-artifact: verctl` and `MISE_ENV=ci,release`, so every release job runs that build.  
`mise.release.toml` is only a `ver` task that runs `verctl` from `PATH`, with no pin.  
Build verctl from source only there, never in every job.  
`.miserc.toml` defaults local `MISE_ENV` to `dev`.

verctl decides how many jobs there are; the workflow file decides where a fixed job runs.  
`ci` and `assets` exist because the number of verify checks or tarballs differs per repo.  
A workflow file is static, so it cannot list a number of jobs that is unknown when it is written.

The release workflow's fixed jobs run on its `runner` input.  
So do not add a `.ctl/ver.yaml` runner key for them, and do not add a `release` runner section.  
Publishing a crate twice is wrong the same way two machines per tarball is wrong.

The changelog adapter does not concatenate Markdown in Rust beyond a typed context.

### Release proofs

`version` rewrites `pins` onto the Version PR commit, because the tag names that commit and publish pushes nothing but the tag.  
Actions run `verctl` from `PATH`.  
`release` is exact-SHA plus a matching per-package CHANGELOG heading.

Before any upload, `release` proves three facts about the commit:

- the default branch contains it;
- its `.github/workflows/` tree, taken at `GITHUB_SHA` when that names a commit and at HEAD otherwise, equals the one at the default tip;
- no package has a skipped version since its last tag.

Only the first two have an `--allow-…` flag, and a run that uses one lists it under `allowed`.  
Cargo and bun are stock recipes, and nothing in the architecture is specific to them.  
Do not name private release skills in this repo.

A breaking library change (one `cargo semver-checks` fails as a patch) targets the `next` branch, which carries the next minor release.  
After each release from `main`, open a PR that merges `main` into `next` with a merge commit.  
That way `next` drops the consumed fragments and builds on the released version.

The version check accepts the versions that PR brings, because `main` carries them at its merge-base with HEAD.

## Git

Conventional commits.  
lefthook runs the hooks.  
No `--no-verify`.  
Branch `type/number-desc`.  
The squash title is the PR title, and its body is the PR description.

## Changes

Plan a behaviour or contract change as an OpenSpec change in `openspec/changes/<name>/` before writing code.  
`openspec/config.yaml` holds this repository's context and rules, and `openspec validate <name>` checks the change.  
One change maps to one `tasks.yaml` row.
