---
"verctl": patch
---

A repo declares the files it serves instead of scripting them. A template in
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

The contract is `docs/served-files.md`, and verctl serves its own
`examples/mise.toml` through it.
