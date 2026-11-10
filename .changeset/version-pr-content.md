---
verctl: minor
verctl:group: changed
---

`verctl check --versions` on a Version PR replays the version step on the merge-base and fails each change the replay does not make.

The error lists every difference.  
Each one is of these kinds:

- a path the version step does not write, such as a source file;
- a changed symlink, compared by its target, and a submodule whose commit changed;
- a manifest key that differs from the planned version and its pins, such as an added dependency;
- a CHANGELOG without the new version's section, or with older text rewritten;
- a fragment the version step consumes that is still present;
- a served file that differs from its template's render;
- a lockfile entry other than a workspace version the plan moves, and a lockfile that does not record each moved version.

A changed `yarn.lock` or `bun.lockb` fails, since verctl does not read them, and so does a changed `pnpm-lock.yaml`, `poetry.lock`, or `pdm.lock`.  
A path that `prepare.stage` names at the merge-base may change, so a staged `yarn.lock` or `bun.lockb` passes.  
A lockfile verctl parses, such as `Cargo.lock`, stays checked even when staged.

The plan, the pins, and `prepare.stage` come from `.ctl/ver.yaml` at the merge-base, so a Version PR cannot widen what it may change.

The replay runs no lockfile owner and no `prepare.after` command, and makes no network call.  
For a Cargo workspace it reads members with `cargo metadata --no-deps --offline`.

When the config at the merge-base does not load, `check --versions` skips the rules that need it and does not fail.  
That happens in the pull request that removes a key this verctl refuses, such as `prepare.version_label`.  
The report then carries one notice per skipped rule, naming the rule and the load error, and JSON holds them under `notices`.  
The config on HEAD must still load.

Earlier releases exempted the Version PR branch whatever it changed.

In the library, `versions::report` and `versions::require` take a `content::Inputs` naming the config file and the fragment directory.  
`content::check` and `content::removes_fragment` are new, with `git::MergeBase`, `git::read_optional`, and `members::versions`.  
`content::check` returns the notices of the rules it skipped, and `versions::VersionReport` gains a `notices` field.
