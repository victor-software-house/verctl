---
verctl: patch
verctl:group: added
---

`assets.dir` and `assets.files` in `.ctl/ver.yaml` pack several files into each package asset tarball, such as a daemon beside its client, a service unit, and a licence. `verctl assets --build` checks that each listed path exists, then packs `tar czf <asset> -C <dir> <files>` with the same placeholders as `binary`. Without them a tarball holds the one binary, as before.
