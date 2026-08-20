---
verctl: patch
---

`tasks/ver/ver` is now served from `.ctl/templates/ver.jinja` and runs
`verctl` from PATH rather than `"$(mise where …)/verctl"`.

`mise where <tool>` resolves a version from the surrounding config, not from
the task's own `#MISE tools` pin, so the pin guaranteed the install and never
the selection. Measured on one machine with 0.0.1 through 0.1.0 installed and
a task pinned to 0.0.4 while the config said 0.1.0: `exec verctl` ran 0.0.4
and `exec "$(mise where …)/verctl"` ran 0.1.0. mise already puts the task's
pinned tool first on PATH, so the plain exec is both simpler and correct.

The two halves ship together on purpose. The committed file said `0.0.1`
while the crate was at `0.1.0`, and only `mise where` was hiding that — so
removing it alone would have frozen `mise run ver` on 0.0.1. Making the file
a template is what keeps the pin current: the Version PR renders it onto the
commit the tag names, so the tag installs itself.
