---
verctl: patch
verctl:group: fixed
---

The served `ver` mise task now runs `raw`. mise relayed its output and appended an SGR reset to every line whenever mise's own colour was on, as under `CLICOLOR_FORCE=1`, so `mise run ver status --format json | jq` received an ESC byte verctl never wrote.
