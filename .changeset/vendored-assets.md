---
verctl: patch
verctl:group: added
---

`vendored.<name>` in `.ctl/ver.yaml` declares release assets built from someone else's source at its own version, and `verctl assets --vendored <name>` (`VERCTL_VENDORED`) plans, builds, and uploads them. `--upload` creates the set's Release when it is missing, never marked latest, and attaches each archive with a `.sha256` written from the digest GitHub reports. A config that vendors may declare no packages. `assets` without `--vendored` is unchanged.
