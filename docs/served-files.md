# Served files: the contract

A **served file** is a file consumers read out of a tag.  
It can be the task file a `?ref=` include fetches, the README line consumers paste, or the catalog of examples they copy from.

Its version must be correct in the tree the tag names, which is the Version PR commit.  
Publish pushes nothing but the tag, so a version written after publish is never served.  
The tag is created at that commit (`target_commitish`), then re-read; a tag that names another commit fails instead of being reused.

This document is the contract for how those files stay correct.  
It is the design `version`, `check`, and `pins` implement; read it before adding a key.

## Goals

1. **The tag is right.**  
   A consumer fetching any tag gets versions that agree with that release, in every file it serves.
2. **A repo declares almost nothing.**  
   Serving one more file adds that file and no config entry.  
   Configuration does not grow with a repo's surface.
3. **Wrong stops the release.**  
   Drift is never served: it fails, naming the file and the field.
4. **One place per fact.**  
   A version lives in a manifest; a served file is generated from it.  
   Nothing is maintained twice.

## Principles

- **Declare, do not script.**  
  A repo states what it serves; verctl decides when and how.
- **Generate what you can, match only what you cannot.**  
  A generated file cannot drift.  
  Patterns exist for files that must be hand-authored.
- **Metadata belongs to the file it describes.**  
  It sits inside the template, so no change has to edit a shared manifest.  
  chezmoi reaches the same conclusion and puts metadata in the file *name*, because it has to know a file's attributes before parsing it.  
  verctl parses every template anyway.
- **Defaults are the convention, and stated.**  
  Every field has a stated meaning when omitted, so a repo that wants the obvious thing writes nothing.
- **Silence is a declaration.**  
  An omitted field means the default, which replaces whatever is there.

## 1. Declarations in the filesystem

`.ctl/` is the directory every ctl CLI shares, so a repo root gains one entry rather than a scatter of tool files.  
Templates live flat in `.ctl/templates/`, and **each one declares where it goes**, in Jinja's own syntax:

```jinja
{%- set path = "tasks/q" -%}
{%- set executable = true -%}
#!/usr/bin/env bash
#MISE tools = { "github:victor-software-house/qctl" = "{{ versions["qctl"] }}" }
```

`q.jinja` above serves `tasks/q/q`.  
Jinja has no frontmatter.  
A top-level `{% set %}` is its equivalent: an export, readable from the evaluated state.  
The metadata therefore travels inside the file it describes.  
No manifest lists templates, no directory tree is mirrored to spell out a path, and nothing sits beside the file it generates.

The whole schema, and what each field means when omitted:

| Export | Type | When omitted |
|:--|:--|:--|
| `path` | directory, relative, never upward | the repository root |
| `name` | one file name | this template's own name, without the suffix |
| `executable` | boolean | `false` (0644) |

`executable` is the only mode there is to declare.  
Git records "only 0755 and 0644 … for regular files" ([gitformat-index]).  
Git's other modes are not files a template renders: 120000 is a symlink, 160000 a gitlink, 040000 a tree.  
There is therefore no `mode` field and no third state.

Silence is a declaration here too.  
A served file whose template omits `executable` is served 0644, and a stale 0755 is corrected.

Declarations are checked once, at the boundary, before anything is written.  
serde parses the exports into one schema, and [garde] validates it.  
Each of `path = 5`, `path = "../.."`, and `name = "tasks/q"` fails the release and names the field.  
The same validators hold `.ctl/ver.yaml` to the same standard.  
A pin that reaches out of the repository fails exactly as such a template would.

Any other export belongs to the template, such as a `tool` name it uses to build a line, and declares nothing.  
Trim declarations with `{%- … -%}`: a shebang has to stay on line one.

Committing one template adds a served file; deleting it retires it.

Only a tracked template renders, because `version` stages what it writes.  
An untracked one would serve a file on the tag with no source beside it, leaving `check` nothing to compare on a fresh clone.  
An untracked template in the source tree therefore **fails the run, naming the file**: commit it or delete it.

A template *outside* the source tree is not verctl's to render.  
That keeps this crate's own changelog templates out of served files.  
verctl skips such a template without a message, because it never claimed to serve anything.

A tree with no repository at all is the one place this rule cannot apply.  
It has no index to leave a file out of, and nothing in it can be served by tag.  
Such a tree renders nothing and reports nothing.

The convention is the default, and a repo can change it:

```yaml
templates:
  source: .ctl/templates   # default
  suffix: .jinja           # default
```

[gitformat-index]: https://git-scm.com/docs/gitformat-index
[garde]: https://docs.rs/garde

## 2. Templates are the default; patterns are the exception

Prefer generating a served file.  
A generated file cannot drift, needs no matching, and states its shape in one place.

Reach for a `pins` entry only for a file that **cannot** be generated:

- The file must lag the release.  
  A repo's own bootstrap pin names a tarball that must already exist.  
  That pin can only move to a version that already shipped.
- A tool with its own format contract owns the file, and a structural edit beats a rendered file.  
  A mise `[tools]` table is the one built-in case.

If a file could be a template, make it one.  
A `pins` entry is a standing admission that a file is hand-authored.

## 3. What a rendering is allowed to be

- **Whole files only.**  
  The template is the file, and the trailing newline is kept.
- **Total.**  
  Undefined behaviour is strict: an unknown name fails the release instead of serving a hole.
- **Idempotent.**  
  The Version PR is regenerated on every push to main, so rendering the same versions twice must produce the same bytes.
- **Context is one map.**  
  `versions["<package name>"]`, keyed by the names in `packages`.  
  No bare `version` shorthand, even for a single-package repo: the served file should name what it pins.
- **Every package is in it**, including the ones this release does not bump.  
  It holds the manifests as they read now, with the release's versions over them.  
  A whole file cannot be rendered from half a map.  
  A served file may name a package no fragment bumped, and then it holds that package's current version.

## 4. What a pattern is allowed to be

A pattern is a named spelling.  
It is declared once and **listed by every file that holds it**.  
A spelling two files share is written once.  
Which files hold it is written down, rather than implied by where an entry sits:

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

- `match` is **literal text** instead of a regex, with `{version}` where the version goes.  
  Exactly one placeholder.  
  `.`, `?`, `//`, and `$` mean themselves, so a pattern is readable by whoever maintains the file it describes.
- A **version** is dotted numerics (`0.0.2`, `1.2.3`, `0.0.10`).  
  Nothing else.
- **Each side of the version needs a boundary.**  
  `{version}` matches digits and dots, so an open side lets a longer version match by its numeric head.  
  `version: {version}` would rewrite `version: 0.1.1-rc1` to `0.1.2-rc1`.
  - A side's literal is the text between `{version}` and the pattern's edge, on that side.
  - A side is bounded when its literal holds at least one character that is not a digit, `.`, `-`, or `+`.  
    A version and its prerelease or build hold only those four.
  - The character next to `{version}` must not be a digit, or a `.` with a digit beyond it.  
    This holds even when the rest of the literal bounds the side.  
    `v{version}.5x` against `v1.2.3.5x` would pin 1.2.3 and keep `.5x`.
  - Under `whole_line`, the line start or end bounds the side, unless the literal on that side is made only of those four characters.

  | Pattern | Result |
  |:--|:--|
  | `name-{version}.tar.gz`, `v{version}.tar`, `download/v{version}-linux` | accepted |
  | `version: {version}` with `whole_line` | accepted |
  | `{version}`, `verctl-{version}`, `verctl@{version}.` | refused |
  | `v{version}-`, `-{version}`, `{version}.1` | refused |
  | `version: {version}-` with `whole_line` | refused |
  | `v{version}.5x`, `v{version}1x`, `x1.{version}`, `x1{version}` | refused |

- Every verb that reads `.ctl/ver.yaml` refuses a pattern with an open side, naming the pattern and the side.  
  With both boundaries, a longer version fails to match, and the arity stops the release.
- **The file cannot decide the boundary for you.**  
  `download/v{version}-linux` against `download/v1.2.3-linux-x64.tar.gz` is correct and pins 1.2.3.  
  A rule that refused every `-` in the file would stop real releases.  
  One case stays open, and only the author can close it: a prerelease that starts with the pattern's own text.  
  `download/v{version}-linux` against `download/v1.2.3-linux.1-linux.tar.gz` pins 1.2.3 and keeps `-linux.1`.
- `whole_line` marks a match that **owns its line** instead of sitting inside one.  
  A frontmatter key or a shell assignment owns its line.  
  Declaring that beats naming a neighbour, because a match that spells out the text after it breaks when that text moves.
- Without `whole_line`, a pattern matches anywhere.  
  That suits an inline spelling: `github:org/tool@1.2.3` is part of a longer line.
- `whole_line` also decides two cases the version alphabet cannot:
  - The same words inside a sentence are not the line.
  - A line ending in `0.1.1-rc1` does not end where the version does, so it fails to match, and the declared arity stops the release.
- `occurrences` is **how often the file must hold the spelling**.  
  A bare count is not always the useful shape, so the vocabulary is explicit:

  | `occurrences` | Means | For |
  |:--|:--|:--|
  | omitted | exactly one | a normal pin (the default) |
  | `"once"` | exactly one | the same thing, said out loud |
  | `"many"` | one or more, count unknown | a document whose examples come and go, where only "still tracked" is worth asserting |
  | `"never"` | none | a spelling that was retired and must stay gone |
  | `{exactly: 2}` | exactly that many | a file that holds it a known number of times |
  | `{at_least: 2}` | that many or more | a floor without a ceiling |

  verctl never infers arity.  
  Each pin declares it, and too few or too many both stop the release.  
  Too few means the pin tracks nothing, and too many means a version spelling nobody accounted for.  
  When a document gains a mention, someone has to raise its arity on purpose.
- `tool` is the one structural form.  
  It covers a mise `[tools]` entry in a file that parses as TOML.  
  It also covers every `?ref=v…` include in that file whose URL path names the tool's own repository.  
  A sibling repo's include is not ours to move.
- A `tool` include whose ref starts with a version and goes on, such as `?ref=v1.2.3-rc1`, fails the release and leaves the file as it was.  
  A ref that is not a version, such as a branch, is not a pin and stays put.

## 5. Failure is loud, and skipping is narrow

Exactly one thing is skipped without an error: a `pins` entry for a package **no fragment bumped**.  
That version did not change, so the file must not either.  
Templates are never skipped.  
Rendering the same versions twice produces the same bytes, and every package's version is in the context (§3).  
A served file is therefore written whole, or the release stops.

Everything else fails the run, before anything is written:

- a pattern whose arity does not match
- a pattern with no placeholder, or more than one
- a `patterns` name nothing under `patterns` declares, or one file lists twice
- a declared pattern no file lists, which is dead configuration
- a `tool` pin whose `[tools]` table or entry is missing
- a template that does not parse, or names something the release does not have
- a template in the source tree that git does not track
- a pin file outside the repository, or reached through a symlink

A partially-rewritten served file is worse than a stopped release, so no file is written until every rewrite in it succeeds.

## 6. Validated at every moment that can serve a wrong version

- **`version`**: rewrites pins and renders templates onto the Version PR commit, the commit the tag will name, and stages them automatically.  
  A repo never lists rendered outputs in `prepare.stage`.
- **`version --dry-run` / `--preview`**: runs the same validation and writes nothing.
- **`check`** runs on every PR and push, with or without fragments.  
  It confirms that patterns match at their declared arity and that every pin names the version on HEAD.  
  It compares every served file with its template's render at those versions.  
  This gate catches a hand edit to a served file.  
  One run names every file that differs.
- **`render`**: rewrites the pins, then renders the templates, at the versions on HEAD.  
  It writes exactly what `check` compares, so it is the fix for every file `check` names.
- **One owner**: a file that a template renders and a `pins` entry also names fails `version`, `check`, and `render`.  
  It fails before anything is written, because `version` would render over the pin's edit.

## 7. Adding, changing, and retiring a site

| Action | Template | Pattern |
|:--|:--|:--|
| add | commit `templates/<target>.jinja` | declare it under `patterns`, list it on the file |
| change | edit the template; never the rendered file | edit `match` |
| move | move the template inside the source tree | edit `file` |
| retire | delete the template | drop the id from the file, and the pattern once nothing lists it |
| carry in a second file | a second template | list the same id on the second `pins` entry |

A hand edit to a rendered file is a mistake that `check` reports and `render` reverts.  
The template is the source.
