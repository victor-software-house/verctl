---
verctl: patch
verctl:group: fixed
---

`verctl publish --plan` accepts a release tag on a commit that HEAD holds only through a merge, when the package's version there equals its version at HEAD. A forkctl append publish leaves the tagged commit only on a merge's second parent, and the plan failed there with "not a first-parent commit". A tag on a commit HEAD does not contain, or at another version, still fails.
