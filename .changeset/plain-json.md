---
verctl: patch
verctl:group: fixed
---

A parse error under `--format json` is now a JSON error even when the bad argument comes before the flag, as in `verctl prepare --bogus --format json`. It printed as text, coloured when `CLICOLOR_FORCE` or `FORCE_COLOR` was set, to a caller that asked for JSON. JSON output stays plain whatever the colour environment says.
