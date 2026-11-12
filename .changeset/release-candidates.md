---
verctl: patch
verctl:group: added
---

A package that sets `prerelease: <tag>` in `.ctl/ver.yaml` releases candidates, such as `0.8.2-rc.1`, then `0.8.2-rc.2`.  
A candidate is published and released as a prerelease: the stock bun upload adds `--tag <tag>`, and its GitHub Release never becomes the latest.  
Pins move to a candidate, because `{version}` and a tool pin's `?ref=v` match a version verctl writes, candidates included.  
Removing the key makes the next Version PR write the stable version, and its CHANGELOG section lists every change the candidates carried.
