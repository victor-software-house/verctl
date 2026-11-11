---
verctl: patch
verctl:group: fixed
---

The publish, asset, and docs jobs of the reusable `release.yml` run the verctl the plan job checked at HEAD.
A pending version commit that pins an older verctl, such as 0.7.4 with no `release` verb, still releases.
