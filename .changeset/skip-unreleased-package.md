---
verctl: patch
---

`verctl publish --plan` and `verctl publish` leave out a package at `0.0.0`, before any registry or GitHub lookup, and `publish` asks no changelog section of it. A crate that adopted the release workflow at `0.0.0` had its placeholder planned as a release, so every push to its default branch ran the publish job on the commit where `0.0.0` first appeared, and the Version PR job never ran.
