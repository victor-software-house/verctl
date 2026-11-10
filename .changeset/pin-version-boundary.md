---
verctl: minor
verctl:group: changed
---

Loading `.ctl/ver.yaml` now refuses a pattern whose `match` leaves either side of `{version}` open, naming the pattern and the side. Before, `version: {version}` against `version: 0.1.1-rc1` rewrote the numeric head and kept `-rc1`.

A side's literal is the text between `{version}` and the pattern's edge, on that side. A side is bounded when its literal holds at least one character that is not a digit, `.`, `-`, or `+`. Under `whole_line: true`, the line start or end bounds the side, unless the literal on that side is made only of those four characters. `name-{version}.tar.gz` loads; `verctl@{version}.` and `v{version}-` do not.

To fix a refused pattern, add the text that follows the version to `match`, such as a closing quote. For a line the pattern owns, set `whole_line: true`.

A `tool` pin now fails, and writes nothing, on an include of its own repository whose ref starts with a version and goes on. Before, `?ref=v1.2.3-rc1` became `?ref=v<new>-rc1`. A ref that is not a version, such as a branch, stays put as before.
