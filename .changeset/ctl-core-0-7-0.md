---
verctl: patch
---

verctl builds on ctl-core 0.7.0, which renders through its own terminal engine and reads `.ctl/ver.yaml` and fragment frontmatter through vsh-serde-saphyr 0.0.3 and yamled 0.0.12.
The help of `--color` and its values, `--no-color`, and the `pretty` format takes ctl-core's colour wording, and in help tables an empty column is 2 cells wide and a wrapped column narrows to its widest line.
