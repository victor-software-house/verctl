---
verctl: minor
verctl:group: changed
---

`verctl version` runs the command that owns each bumped manifest's lockfile and commits that lockfile without a `prepare.stage` entry.

These commands own the lockfiles:

- `cargo update --workspace` beside `Cargo.lock`;
- `uv lock` beside `uv.lock`;
- the package manager's install beside a JavaScript lockfile.

A package's own `after` runs as well; earlier releases only listed these commands.  
`prepare.after` runs after them, and a `prepare.after` that repeats an owner's command in the same directory runs once.  
The report's `after` lists each command that ran.

The next Version PR consumes a fragment that bumps nothing, such as one holding only `demo: none`.  
Earlier releases left it pending.

In the library, `prepare::apply_plan` returns no follow-up commands.  
`prepare::steps` and `prepare::run_steps` list and run them, and `detect::owner` names a manifest's lockfile owner, its directory, and its lockfile.
