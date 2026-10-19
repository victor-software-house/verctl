---
verctl: patch
---

An asset upload that GitHub refuses because the release already has a complete asset of that name now reports `already` instead of failing. Two release runs for the same commit both build every asset, so the run that uploads second used to fail after a release that was in fact complete. The duplicate check read the error's text, which never contained GitHub's `already_exists` code; it now reads the code from the response. When the other run is still sending the asset, the upload waits up to a minute for it to finish. An asset whose earlier upload broke off still fails, with a message to delete it and rerun.
