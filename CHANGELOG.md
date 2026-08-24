# Changelog

## verctl 0.3.1

- Render the operator skill and installed instructions from ctl-core Surface fragments, keeping Clap as the only command inventory and the shared mounted invocation as the only no-`--` rule.

## verctl 0.3.0

- Route every command through ctl-core's typed App and semantic Document presentation kernel, so pretty, colorless, JSON, help, errors, and exit codes share one report model.

## verctl 0.2.4

- The operator skill template wraps Jinja examples in `raw` so `prepare` can render it.
- Served `ver` task mounts the ctl-core Usage spec. `mise run ver status` needs no `--`.

## verctl 0.2.3

- `verctl publish` fetches the one missing default-branch ref with its history, not at depth 1, so the ancestry proof can walk parents. It rewrites `origin/HEAD` only from `VERCTL_DEFAULT_BRANCH`, never from `GITHUB_BASE_REF`.

## verctl 0.2.2

- `verctl publish` fetches `origin/<default>` at depth 1 only when that ref is missing. GitHub git HTTPS authenticates as `x-access-token`; the token is never in the URL and never sent as Bearer. The publish action no longer shells out to `git fetch`.

## verctl 0.2.1

- `actions/publish` fetches the default branch at depth 1 before pointing `origin/HEAD` at it, so a shallow checkout of the merge SHA still has that ref.

## verctl 0.2.0

- Declare release tag names with `tags.template` (default `v{version}`). A template with `{name}` creates one tag and one Release per package, each filled from that package's CHANGELOG. Without `{name}`, differing versions refuse instead of guessing the first package.
- Pin each GitHub Release tag to the published HEAD commit. After create, and when a Release already exists, the tag is re-read and the run fails if it names another commit.

## verctl 0.1.3

- `docs/served-files.md` §4 no longer promises that a prerelease always fails the
release, because it does not. The condition is exact: a match ending in
literal text has a right-hand boundary, so `verctl@{version} today` reads
`verctl@1.0.0-rc.1 today` as no match at all. A match ending *at* the
placeholder has none, and rewrites the numeric head, leaving the tail.

The doc now says that, and says what to do about it — put the following text
into `match`, or use `whole_line` for a line the match owns. It also records
why nothing can be inferred: `download/v{version}` against
`download/v1.2.3-linux-x64.tar.gz` is correct and pins 1.2.3, so a rule
refusing every trailing `-` would stop real releases.

## verctl 0.1.2

- The bundled skill now says which release its text describes.
`skills/verctl/SKILL.md` carries `version:` in its frontmatter, kept current
by a `patterns` pin, so the Version PR moves it onto the commit the tag names
along with every other version site.

A skill is prose, not a generated file, so it takes a pattern rather than a
template — a template would claim to own the whole document. The copy vendored
into the global skill set tracks `latest-release`, so what a consumer loads
now names the binary they actually have.

A pattern can now say `whole_line: true`, meaning the match owns its line
rather than sitting inside one. Unsaid, nothing changes: an inline spelling
like `github:org/tool@1.2.3` still matches anywhere. Saying it beats naming
the text next door — a match that spells out its neighbour breaks when the
neighbour moves — and it decides two cases the version alphabet cannot: the
same words inside a sentence are not the line, and a line ending in
`0.1.1-rc1` does not end where the version does, so it fails to match and the
declared arity stops the release.

## verctl 0.1.1

- `tasks/ver/ver` is now served from `.ctl/templates/ver.jinja` and runs
`verctl` from PATH rather than `"$(mise where …)/verctl"`.

`mise where <tool>` resolves a version from the surrounding config, not from
the task's own `#MISE tools` pin, so the pin guaranteed the install and never
the selection. Measured on one machine with 0.0.1 through 0.1.0 installed and
a task pinned to 0.0.4 while the config said 0.1.0: `exec verctl` ran 0.0.4
and `exec "$(mise where …)/verctl"` ran 0.1.0. mise already puts the task's
pinned tool first on PATH, so the plain exec is both simpler and correct.

The two halves ship together on purpose. The committed file said `0.0.1`
while the crate was at `0.1.0`, and only `mise where` was hiding that — so
removing it alone would have frozen `mise run ver` on 0.0.1. Making the file
a template is what keeps the pin current: the Version PR renders it onto the
commit the tag names, so the tag installs itself.
- A template in the source tree that git does not track now fails the run,
naming the file, instead of rendering nothing and letting the release serve
the stale hand-authored version of that file. Adding a served file and
getting silence was the one place this lane failed open, and it did it once
per repo, at the moment somebody was learning the feature.

Only tracked templates still render, for the reason they always did:
`prepare` stages what it writes, so a template git does not carry would put
a served file on the tag with no source beside it, and `check` would have
nothing to compare against on a fresh clone. What changed is the answer to
one that is present anyway — commit it or delete it.

Two things stay silent, both deliberately. A template *outside* the source
tree is not verctl's to render, which is what keeps this crate's own
changelog templates out of the released set; it was never a claim to serve
anything. And a tree with no repository has no index to not carry a file,
and cannot serve anything by tag, so it renders nothing without a word.

An ignored template is left alone rather than refused: ignoring a file is a
repo saying it is not part of what it publishes, which is the same statement
the index makes. A symlink in the source tree is not followed either, so a
directory outside the repository cannot have its templates counted as this
one's.

## verctl 0.1.0

- A repo now declares everything in `.ctl/ver.yaml`, and served-file templates
live in `.ctl/templates/`. `verctl.toml` is not read at all — no fallback, no
deprecation window. `.ctl/` is the directory every ctl CLI shares, so a repo
that uses two of them still gains one entry at its root, and templates sit in
one place because a template already declares its own target and nothing in one
is verctl's except who renders it.

YAML, not TOML, so every file a repo writes for a ctl CLI is the language
`tasks.yaml` already is. The sections are unchanged and so is every rule across
them: a pattern no pin lists still fails the load, and a job or asset target
naming an undeclared machine still fails with the declared names. The one
spelling that moved is an arity with a bound, now `{exactly: 2}` and
`{at_least: 2}`.

Every `path` in the file is relative to the directory that holds `.ctl/`, not
to the directory the file sits in, so `-c crates/foo/.ctl/ver.yaml` still
governs `crates/foo`. Complaints name sections the way a repo writes them:
`prepare.stage`, `patterns.install`, `not declared in runners`.

The retired `[assets].targets` shim is gone with it. Its own removal condition
was that every consumer had bumped past the release that replaced it, and this
is that bump.

## verctl 0.0.4

- The cargo driver's lockfile follow-up is `cargo update --workspace`, not
`cargo generate-lockfile`. A bump makes one line of `Cargo.lock` stale;
`generate-lockfile` re-resolved all 138 packages and moved a dependency the
bump never touched.

Lockfile detection now stops at the repository root for cargo and JavaScript
alike. Both branches walk one shared scope, so a `Cargo.lock` or `bun.lock`
in some unrelated parent directory can no longer claim a manifest that is not
part of it.
- `prepare` rewrites `[[pins]]` and puts them on the Version PR commit. The
tag names that commit, so the released tree carries pins that name the
release; a pin rewritten after publish can never reach the tree a consumer
fetches by tag or `?ref=`. Publish pushes nothing but the tag.

A pin that has to name an already-published tarball — the one a repo's own
release lane installs — is not a `[[pins]]` entry, and this repo's
`mise.release.toml` says so.
- A repo declares the files it serves instead of scripting them. A template in
`.verctl/templates/` says inside itself where it goes — `path`, `name`, and
`executable` are top-level Jinja exports, parsed into one schema and validated
at the boundary, so nothing sits beside the file it generates and no manifest
lists templates. Committing one adds a served file; only tracked templates
render, so scratch work renders nowhere. `prepare` writes them onto the Version
PR commit, which is the commit the tag names.

For a file that has to be hand-authored, `[[pins]]` now rewrites any format. A
version spelling is named once as `[patterns.<id>]` and listed by every file
that carries it, so two files that say the same thing say it in one place and
which file carries a spelling is written down rather than implied by where a
table sits. Each pattern declares how often its file must say it — `once`,
`many`, `never`, `{ exactly = N }`, `{ at_least = N }` — and both too few and
too many stop the release, as does a name nothing declares, a name listed
twice, and a declared pattern no file lists.

`verctl.toml` is now held to the same standard by the same validators, so the
file a repo writes fails the way a template does — naming the field, in words
that say what to change. A config that names no package, a runner with no
label, and an empty `[templates].suffix` are rejected at load rather than
carried into a release.

Two corrections that come with it: a project in a subdirectory of a repository
now serves its own templates instead of none, and templates render from every
package's version — the manifests as they read now, with the release's bumps
over them — so a served file may mention a package this release did not bump.

The contract is `docs/served-files.md`, and verctl serves its own
`examples/mise.toml` through it.

## verctl 0.0.3

- `prepare --pr` commits dirty paths matching `[prepare].stage` from the same git-status walk as the unexpected-dirty check, including deletions. `[prepare].stage_ignored` opts into gitignored matches.
- Runners are configuration. A machine is declared once as `[runners.NAME]` with
`labels`, the literal GitHub label list — every label at once, so one entry is
one machine however many labels it carries. Jobs name machines: `[ci.NAME]`
takes `runners = [...]` and gets one check per machine, `[assets.NAME]` takes a
single `runner` because one tarball is one machine. A name resolves against
`[runners]` or fails listing what is declared; only labels reach `runs-on:`, so
verctl never invents a label and a label can never pass as a machine name.
Omitting `[ci]` runs one `verify` job on `ubuntu-latest`, so a repo that says
nothing behaves as before. New `verctl ci` writes the matrix a `plan` job hands
to `runs-on:`.

`verctl instructions` now states which jobs take a runner and why: verctl
decides how many jobs there are, the workflow file decides where a fixed job
runs. `[ci]` and `[assets]` exist because only the repo knows how many verify
checks or tarballs it wants. `plan`, `crate`, `pin`, and `prepare` are always
one job each, so their `runs-on` stays a literal in the workflow you own. There
is no `[release]` runner table and none is planned.

`$GITHUB_OUTPUT` is appended to rather than truncated, so a plan step no
longer drops assignments an earlier step in the same step file made.

Breaking for anyone who wrote an asset target. `[assets] targets = [...]` is
retired for one `[assets.NAME]` table per target, and naming it is an error
that states the migration:

```toml
# before
[assets]
targets = ["linux-x64"]

# after — built-in name, so nothing else is needed
[assets.linux-x64]

# after — with a machine of your own
[runners.big]
labels = ["self-hosted", "linux", "x64"]

[assets.linux-x64]
runner = "big"
```

An unknown target name with a partial record now fails instead of building
against an empty target triple. `{runner}` is no longer expanded in `[assets]`
`prepare` / `build` / `binary` and has no replacement, since a label list has
no single rendering. A workflow copied from `examples/workflows/publish.yml`
renames `matrix.runner` to `matrix.labels` in the same step as the bump.
- `check --versions` treats the GitHub event label `verctl:version` as the Version PR. `prepare --pr` applies that label. Not `GITHUB_HEAD_REF`.

## verctl 0.0.2

- Actions run the released tarball, mise splits dev/release/ci, publish is exact-SHA plus a matching changelog heading, `check --versions` blocks hand edits, and `verctl pin` rewrites collocated github:verctl refs after the tarball exists.

## verctl 0.0.1

- First release. Version PR via git2 and octocrab, ctl-core chassis.

