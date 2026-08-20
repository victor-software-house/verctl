---
verctl: patch
---

The bundled skill now says which release its text describes.
`skills/verctl/SKILL.md` carries `version:` in its frontmatter, kept current
by a `patterns` pin, so the Version PR moves it onto the commit the tag names
along with every other version site.

A skill is prose, not a generated file, so it takes a pattern rather than a
template — a template would claim to own the whole document. The copy vendored
into the global skill set tracks `latest-release`, so what a consumer loads
now names the binary they actually have.
