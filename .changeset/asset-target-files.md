---
verctl: patch
verctl:group: added
---

An `assets` target may list its own `files`, which replace `assets.files` for that target alone.  
A release whose platforms ship different files, such as a tool built only on Linux, packs each list from the shared `assets.dir`.  
A target without `files` packs the shared list, as before.  
The load names the target when its `files` comes without `assets.dir`, is empty, or sits beside `assets.binary`.
