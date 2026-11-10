---
verctl: minor
verctl:group: changed
---

`verctl check` now renders every template and rewrites every `pins` entry in memory, at the versions the manifests hold, with or without pending fragments.  
It fails on each served file that differs from its template.  
It also fails on each pin that names another version, or whose pattern matches nothing.  
One run names every such file.

A pull request that edits a served file by hand, or changes a template without rendering it, fails.

`verctl render` renders every template after it rewrites the pins.  
So it writes exactly what `check` compares, and it is the fix for a failed check.  
`check` also reads `.ctl/ver.yaml` when no fragment is pending.

A file that a template renders and a `pins` entry also names has two owners.  
`verctl version`, `check`, and `render` refuse it before writing anything, naming the file, the template, and the `pins` entry.

In the library, `templates::plan` and `templates::write` take the `pins` as an argument, so they can name that conflict.
