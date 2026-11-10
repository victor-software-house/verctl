---
verctl: patch
verctl:group: changed
---

`verctl instructions` and the bundled skill are split into headed sections, each sized to read on its own.  
Both open with the release workflow in four steps and a table that maps each `.ctl/ver.yaml` key to its section.

The instructions, the skill, and the README now say that the release workflow's `plan`, `publish`, `docs`, and `propose` jobs run on its `runner` input.  
Before, they named jobs the workflow does not have.
