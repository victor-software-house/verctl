---
verctl: patch
verctl:group: fixed
---

Each release asset job keys its build cache by its target.
A cross build such as `linux-arm64` runs on the same runner type as `linux-x64`, so both jobs saved one key, and the job that lost the race failed the release.
