---
verctl: minor
---

Declare release tag names with `tags.template` (default `v{version}`). A template with `{name}` creates one tag and one Release per package, each filled from that package's CHANGELOG. Without `{name}`, differing versions refuse instead of guessing the first package.
