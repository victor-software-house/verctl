---
verctl: patch
verctl:group: fixed
---

A new release builds its asset tarballs again.
0.8.1 read the targets of the version commit's config from an `assets.targets` key, but targets sit directly under `assets`, so a release that held no asset yet was checked for none and released without tarballs.
The next release run on the default branch builds the tarballs a release still lacks, 0.8.1's among them.
