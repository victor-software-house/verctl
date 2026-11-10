---
verctl: minor
verctl:group: changed
---

`verctl check --versions` on a Version PR replays the version step on the merge-base and fails each change the replay does not make.

The error lists every difference.  
Each one is of these kinds:

- a path the version step does not write, such as a source file;
- a manifest key that differs from the planned version and its pins, such as an added dependency;
- a CHANGELOG without the new version's section, or with older text rewritten;
- a fragment the version step consumes that is still present;
- a served file that differs from its template's render;
- a lockfile entry other than a workspace version the plan moves, and a lockfile that does not record each moved version.

A changed `yarn.lock` or `bun.lockb` fails, since verctl does not read them, and so does a changed `pnpm-lock.yaml`, `poetry.lock`, or `pdm.lock`.  
A path that `prepare.stage` names at the merge-base may change.  
The plan, the pins, and `prepare.stage` come from `.ctl/ver.yaml` at the merge-base, so a Version PR cannot widen what it may change.

The replay runs no lockfile owner and no `prepare.after` command, and makes no network call.  
For a Cargo workspace it reads members with `cargo metadata --no-deps --offline`.

Earlier releases exempted the Version PR branch whatever it changed.

In the library, `versions::report` and `versions::require` take a `content::Inputs` naming the config file and the fragment directory.  
`content::check` and `content::removes_fragment` are new, with `git::MergeBase`, `git::read_optional`, and `members::versions`.
