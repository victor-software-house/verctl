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

A pattern can now say `whole_line: true`, meaning the match owns its line
rather than sitting inside one. Unsaid, nothing changes: an inline spelling
like `github:org/tool@1.2.3` still matches anywhere. Saying it beats naming
the text next door — a match that spells out its neighbour breaks when the
neighbour moves — and it decides two cases the version alphabet cannot: the
same words inside a sentence are not the line, and a line ending in
`0.1.1-rc1` does not end where the version does, so it fails to match and the
declared arity stops the release.
