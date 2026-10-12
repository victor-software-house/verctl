---
verctl: minor
---

A fragment may set `verctl:group` to `added`, `changed`, or `fixed`. Once one fragment in a release section names a group, the section lists its entries under `### Added`, `### Changed`, and `### Fixed` headings, and a fragment without the key goes under `### Changed`. A section where no fragment names a group renders as before. `Fragment` has a new public field, `group`.
