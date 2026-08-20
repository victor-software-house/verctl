---
verctl: minor
---

A repo now declares everything in `.ctl/ver.yaml`, and served-file templates
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
