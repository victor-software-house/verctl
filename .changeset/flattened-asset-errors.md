---
verctl: patch
verctl:group: fixed
---

A problem in an asset target names the path `.ctl/ver.yaml` holds, such as `assets.linux-x64.files`.  
It lands on the line of that key instead of the `assets:` line.
