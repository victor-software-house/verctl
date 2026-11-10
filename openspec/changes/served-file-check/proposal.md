# Check served files on every pull request

## Why

[docs/served-files.md](../../../docs/served-files.md) section 6 says `check`
compares every served file to what its template renders at the versions on
HEAD. It does not. `verctl check` with no fragments returns before it reads
the config, and with fragments it only plans the bumps. A served file edited
by hand passes every pull request and is overwritten at the next release,
or ships wrong when no release touches it.

A file that a template renders and a `pins` entry also names has two owners.
`version` rewrites pins first and renders templates second, so the template
overwrites the pin's edit without a word.

The operator placed this check in the 0.8.0 minor on 2026-11-09, because it
fails pull requests that pass today (operator, 2026-11-09).

Queue row: VER-020.

## What changes

1. **BREAKING:** `verctl check` renders every template at the versions on
   HEAD and fails on each served file whose bytes differ, naming the file and
   its template. It does so with or without fragments.
2. **BREAKING:** `verctl check` fails when a `pins` entry no longer matches
   at its declared count, or holds a version other than the one on HEAD.
3. **BREAKING:** `version`, `check`, and `render` fail before writing
   anything when a template's target is also a `pins` file. The message
   names the file and both declarations.
4. **BREAKING:** `verctl render` rewrites the `pins` entries, then renders
   every template, at the versions on HEAD. It writes what `check`
   compares, so it is the one command that fixes a failed check.
5. `check --versions` is unchanged.

## Impact

1. Code: `src/main.rs` (`check_report`, `render_report`),
   `src/templates.rs`, `src/pins.rs`.
2. Docs: `docs/served-files.md` section 6 becomes true; `src/instructions.md`,
   `skills/verctl/SKILL.md`, and `AGENTS.md` say what `check` verifies.
3. A repository with a hand-edited served file fails `check` on its next pull
   request. The fix is `verctl render`.
