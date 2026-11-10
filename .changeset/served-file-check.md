---
verctl: minor
verctl:group: changed
---

`verctl check` now renders every template and rewrites every `pins` entry in memory, at the versions the manifests hold, with or without pending fragments. It fails on each served file that differs from its template, and on each pin that names another version or whose pattern no longer matches, and names every such file in one run. A pull request that edits a served file by hand, or changes a template without rendering it, now fails.

`verctl render` now renders every template after it rewrites the pins, so it writes exactly what `check` compares and is the fix for a failed check. `check` also reads `.ctl/ver.yaml` when no fragment is pending.

A file that a template renders and a `pins` entry also names has two owners. `verctl version`, `check`, and `render` now refuse it before writing anything, naming the file, the template, and the `pins` entry.
