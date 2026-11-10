# Refuse a pattern that cannot bound the version

## Why

A pattern's `{version}` matches dotted numerics.
When nothing marks where the version ends, a longer version matches by its numeric head.
With `match: "version: {version}"` and a file reading `version: 0.1.1-rc1`, `prepare` exited 0 and wrote `version: 0.1.2-rc1` (measured 2026-08-20, row note).
The release carried on with a half-rewritten pin.

A match that ends in literal text already refuses, through the arity check.
`whole_line: true` supplies the boundary for a line the match owns.
What remains is an inline pattern with no text on one side of `{version}`, or with a digit or `.` next to it.
The same holds on the left: `{version} today` against `1.0.0-rc.1.2 today` matches `1.2 today`.

A lone `-` or `+` after the placeholder fails the same way, since it opens a prerelease or build: `v{version}-` against `v1.2.3-rc1` matches `1.2.3-`.
The `tool` form has the defect too: an include ending `?ref=v1.2.3-rc1` would become `?ref=v<new>-rc1`.

The next character in the file cannot decide it.
`download/v{version}` against `download/v1.2.3-linux-x64.tar.gz` is correct and pins 1.2.3, and `-linux-x64.tar.gz` parses as a semver prerelease.
Only the author knows where the version ends, so the author states it.

The operator placed this break in the 0.8.0 minor (operator, 2026-11-10).

Queue row: [VER-031][ledger].

[ledger]: ../../../tasks.yaml

## What changes

1. **BREAKING:** loading `.ctl/ver.yaml` fails on a pattern with an open side of `{version}`.
   A side's literal is the text between `{version}` and the pattern's edge, on that side.
   A side is bounded when both of these hold:
   - its literal holds at least one character that is not a digit, `.`, `-`, or `+`;
   - its character next to `{version}` is neither a digit nor a `.` with a digit beyond it.

   Under `whole_line: true`, the line start or end bounds the side, unless that literal is only those four.
2. **BREAKING:** a `tool` pin fails when its own include's `?ref=v` starts with a version and goes on, and leaves the file untouched.
   A ref that is not a version, such as a branch, stays put as before.
3. A test writes a prerelease into a pinned file and asserts that `version` fails and leaves the file untouched.
4. [docs/served-files.md](../../../docs/served-files.md) section 4 states the enforced rule in place of the advice it gives now.

## Impact

1. Code: `src/config.rs` (pattern validation), `src/pins.rs` (the include rewrite and tests), `tests/served.rs`.
2. Docs: `docs/served-files.md` section 4, `examples/ver.yaml`, `src/instructions.md`, `skills/verctl/SKILL.md`, rustdoc on `PinPattern`.
3. Callers: qctl, forkctl, and rgbctl each need a `.ctl/ver.yaml` edit before they take 0.8.0.
   The edits work on verctl 0.7.2, which they run today.
   ctl-core and shrub already pass, and yamled and unixd declare no patterns.
