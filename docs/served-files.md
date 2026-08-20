# Served files: the contract

A **served file** is a file consumers read out of a tag: the task file a
`?ref=` include fetches, the example catalog someone copies, the README line
they paste. Its version must be correct in the tree the tag names, which is the
Version PR commit — publish pushes nothing but the tag, so a version written
after publish is never served.

This document is the contract for how those files stay correct. It is the design
`prepare`, `check`, and `pins` implement; read it before adding a key.

## Goals

1. **The tag is right.** A consumer fetching any tag gets versions that agree
   with that release, in every file it serves.
2. **A repo says almost nothing.** Serving one more file is one more file, not
   another config entry — configuration does not grow with a repo's surface.
3. **Wrong stops the release.** Drift is never served: it fails, naming the file
   and the field.
4. **One place per fact.** A version lives in a manifest; a served file is
   generated from it. Nothing is maintained twice.

## Principles

- **Declare, do not script.** A repo states what it serves; verctl decides when
  and how.
- **Generate what you can, match only what you cannot.** A generated file cannot
  drift. Patterns exist for files that must be hand-authored.
- **Metadata belongs to the file it describes** — inside the template, not in a
  shared manifest that every change has to edit. (chezmoi reaches the same
  conclusion and puts it in the file *name*; it has to know a file's attributes
  before parsing it, and verctl parses every template anyway.)
- **Defaults are the convention, and stated.** Every field says what it means
  unsaid; a repo that wants the obvious thing writes nothing.
- **Silence is a declaration.** Unsaid means the default, not "leave whatever is
  there".

## 1. The filesystem declares, not the config

`.ctl/` is the directory every ctl CLI shares, so a repo root gains one
entry rather than a scatter of tool files. Templates live flat in
`.ctl/templates/`, and **each one declares where it goes**, in Jinja's own
syntax:

```jinja
{%- set path = "tasks/q" -%}
{%- set executable = true -%}
#!/usr/bin/env bash
#MISE tools = { "github:victor-software-house/qctl" = "{{ versions["qctl"] }}" }
```

`q.jinja` above serves `tasks/q/q`. Jinja has no frontmatter, but a top-level
`{% set %}` is its equivalent — an export, readable from the evaluated state —
so the metadata travels inside the file it describes. No manifest lists
templates, no directory tree is mirrored to spell out a path, and nothing sits
beside the file it generates.

The whole schema, and what each field means unsaid:

| Export | Type | Unsaid |
|:--|:--|:--|
| `path` | directory, relative, never upward | the repository root |
| `name` | one file name | this template's own name, without the suffix |
| `executable` | boolean | `false` — 0644 |

`executable` is the only mode there is to declare: git records "only 0755 and
0644 … for regular files" ([gitformat-index]), and its other modes are not files
a template renders — 120000 is a symlink, 160000 a gitlink, 040000 a tree. So
there is no `mode` field and no third state. Silence is a declaration, not an
exception: a served file whose template says nothing is served 0644, and a stale
0755 is corrected.

Declarations are checked at the boundary, once, before anything is written:
serde parses the exports into one schema and [garde] validates it, so
`path = 5`, `path = "../.."`, and `name = "tasks/q"` each fail the release
naming the field. `.ctl/ver.yaml` is held to the same standard by the same
validators — a pin that reaches out of the repository fails exactly the way a
template that does would. Any other export is the template's own business — a `tool`
name it uses to build a line is not a declaration.

Trim declarations with `{%- … -%}`: a shebang has to stay on line one.


Committing one template adds a served file; deleting it retires it. Only a
tracked template renders, because `prepare` stages what it writes: one git does
not carry would put a served file on the tag with no source beside it, and
`check` would have nothing to compare against on a fresh clone. An untracked
template in the source tree therefore **fails the run, naming the file** —
commit it or delete it. A template *outside* the source tree is not verctl's to
render, which is what keeps this crate's own changelog templates out of this, and
that one is silent because it was never a claim to serve anything.

A tree with no repository at all is the one place this cannot apply: there is no
index to not carry a file, and nothing can be served by tag anyway, so such a
tree renders nothing and says nothing.

The convention is the default, not the only option:

```yaml
templates:
  source: .ctl/templates   # default
  suffix: .jinja           # default
```

[gitformat-index]: https://git-scm.com/docs/gitformat-index
[garde]: https://docs.rs/garde

## 2. Templates are the default; patterns are the exception

Prefer generating a served file. A generated file cannot drift, needs no
matching, and states its shape in one place.

Reach for a `pins` entry only for a file that **cannot** be generated:

- it must lag the release (a repo's own bootstrap pin names a tarball that has
  to exist already, so it can only move to a version that already shipped);
- it is owned by a tool with its own format contract, where a structural edit
  beats a rendered file — a mise `[tools]` table is the one built-in case.

If a file could be a template, make it one. A `pins` entry is a standing
admission that a file is hand-authored.

## 3. What a rendering is allowed to be

- **Whole file, not a fragment.** The template is the file; the trailing
  newline is kept.
- **Total.** Undefined behaviour is strict: an unknown name fails the release
  instead of serving a hole.
- **Idempotent.** The Version PR is regenerated on every push to main, so
  rendering the same versions twice must produce the same bytes.
- **Context is one map.** `versions["<package name>"]`, keyed by the names in
  `packages`. No bare `version` shorthand, even for a single-package repo:
  the served file should name what it pins.
- **Every package is in it**, not only the ones this release bumps: the
  manifests as they read now, with the release's versions over them. A whole
  file cannot be rendered from half a map, so a served file may mention a
  package no fragment bumped and still say that package's current version.

## 4. What a pattern is allowed to be

A pattern is a spelling, not a file. It is named once and **listed by every file
that carries it**, so a spelling two files share is written once and which file
carries it is written down rather than implied by where an entry sits:

```yaml
patterns:
  install:
    match: "github:victor-software-house/verctl@{version}"
    occurrences: once

pins:
  - file: README.md
    package: verctl
    patterns: [install]
  - file: docs/install.md
    package: verctl
    patterns: [install]
```

- `match` is **literal text**, not a regex, with `{version}` where the version
  goes. Exactly one placeholder. `.`, `?`, `//`, and `$` mean themselves, so a
  pattern is readable by whoever maintains the file it describes.
- A **version** is dotted numerics (`0.0.2`, `1.2.3`, `0.0.10`). Nothing else.
- **Give the version a right-hand boundary.** A match ending in literal text has
  one, so `verctl@{version} today` reads `verctl@1.0.0-rc.1 today` as no match
  at all and the arity stops the release. A match ending *at* the placeholder
  has none, and will rewrite the numeric head of a longer version — leaving the
  tail. `whole_line` is the boundary for a line the match owns; otherwise put
  the text that follows into `match`. Nothing can be inferred for you here:
  `download/v{version}` against `download/v1.2.3-linux-x64.tar.gz` is correct
  and pins 1.2.3, so a rule that refused every trailing `-` would stop real
  releases.
- `whole_line` says the match **owns its line** rather than sitting inside one.
  Unsaid, a pattern matches anywhere, which is what an inline spelling wants —
  `github:org/tool@1.2.3` is part of a longer line. A frontmatter key or a
  shell assignment is not: it owns its line, and saying so beats naming a
  neighbour, because a match that spells out the text after it breaks when that
  text moves. Owning the line also decides two cases the version alphabet
  cannot: the same words inside a sentence are not the line, and a line ending
  in `0.1.1-rc1` does not end where the version does, so it fails to match and
  the declared arity stops the release.
- `occurrences` is **how often the file must say it**. A bare count is not
  always the useful shape, so the vocabulary is explicit:

  | `occurrences` | Means | For |
  |:--|:--|:--|
  | omitted | exactly one | a normal pin — the default |
  | `"once"` | exactly one | the same thing, said out loud |
  | `"many"` | one or more, count unknown | a document whose examples come and go, where only "still tracked" is worth asserting |
  | `"never"` | none | a spelling that was retired and must stay gone |
  | `{ exactly = 2 }` | exactly that many | a file that says it a known number of times |
  | `{ at_least = 2 }` | that many or more | a floor without a ceiling |

  Arity is declared, never inferred, and both directions stop the release: too
  few means the pin no longer tracks anything, too many means a version
  spelling nobody accounted for. A document that grows a mention is a decision
  someone makes, not a silent rewrite.
- `tool` is the one structural form: a mise `[tools]` entry in a file that
  parses as TOML, plus every `?ref=v…` include in that file whose URL path
  names the tool's own repository. A sibling repo's include is not ours to move.

## 5. Failure is loud, and skipping is narrow

Exactly one thing is silently skipped: a `pins` entry for a package **no
fragment bumped**. That version did not change, so the file must not either.
Templates are not skipped — rendering the same versions twice produces the same
bytes, and they are rendered from every package's version (§3), so a served file
is written whole or the release stops.

Everything else fails the run, before anything is written:

- a pattern whose arity does not match
- a pattern with no placeholder, or more than one
- a `patterns` name nothing under `patterns` declares, or one file lists twice
- a declared pattern no file lists — dead configuration, not a spare
- a `tool` pin whose `[tools]` table or entry is missing
- a template that does not parse, or names something the release does not have
- a template in the source tree that git does not track
- a pin file outside the repository, or reached through a symlink

A partially-rewritten served file is worse than a stopped release, so no file is
written until every rewrite in it succeeds.

## 6. Validated at every moment that can serve a wrong version

- **`prepare`** — rewrites pins and renders templates onto the Version PR
  commit, the commit the tag will name, and stages them automatically. A repo
  never lists rendered outputs in `prepare.stage`.
- **`prepare --dry-run` / `--preview`** — runs the same validation and writes
  nothing.
- **`check`** — every PR and push: patterns still match at their declared
  arity, and every served file still equals what its template renders at the
  versions on HEAD. This is the gate that catches a served file edited by hand
  instead of through its template.

## 7. Adding, changing, and retiring a site

| Action | Template | Pattern |
|:--|:--|:--|
| add | commit `templates/<target>.jinja` | declare it under `patterns`, list it on the file |
| change | edit the template; never the rendered file | edit `match` |
| move | move the template inside the source tree | edit `file` |
| retire | delete the template | drop the id from the file, and the pattern once nothing lists it |
| carry in a second file | a second template | list the same id on the second `pins` entry |

A hand edit to a rendered file is a mistake `check` reports, not a supported
workflow: the template is the source.
