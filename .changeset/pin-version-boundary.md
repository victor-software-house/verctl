---
verctl: minor
verctl:group: changed
---

Loading `.ctl/ver.yaml` now refuses a pattern whose `match` leaves either side of `{version}` open, naming the pattern and the side. Before, `version: {version}` against `version: 0.1.1-rc1` rewrote the numeric head and kept `-rc1`.

A side has a boundary in one of three ways:

- a character next to `{version}` other than a digit, `.`, `-`, or `+`;
- after the version, a `-` or `+` with more text after it;
- with `whole_line: true`, the line start or the line end.

To fix a refused pattern, add the text that follows the version to `match`, such as a closing quote. For a line the pattern owns, set `whole_line: true`.

A `tool` pin now fails, and writes nothing, on an include of its own repository whose ref starts with a version and goes on. Before, `?ref=v1.2.3-rc1` became `?ref=v<new>-rc1`. A ref that is not a version, such as a branch, stays put as before.
