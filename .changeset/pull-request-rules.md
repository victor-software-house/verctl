---
verctl: minor
verctl:group: changed
---

`verctl check --versions` fails a pull request that is not the Version PR in two more cases.

Each case is a change the pull request makes:

- It adds a CHANGELOG section for a version.  
  A heading names its version last, as in `## demo 1.2.0` or `## 1.2.0`.  
  A version the CHANGELOG held at the merge-base is not added.  
  Nor is one the release branch held at its merge-base with HEAD.  
  Fixing the name in a heading therefore passes, and so does merging the release branch.
- It changes whether a package publishes and adds no fragment that bumps that package above `none`.  
  Whether a package publishes comes from its `publisher`, the members a workspace package uploads, a Cargo `publish` key (inherited from the workspace root with `publish.workspace`), and `private` in `package.json`.  
  A setting the release branch already holds at its merge-base with HEAD passes, as a released version does.

For a pull request with either problem, `check` looks up on GitHub whether it is the Version PR, as for one that changes a version.  
A merge queue run skips both rules.

`members::members` reads the workspace with `cargo metadata --offline`, so it never reaches a registry.  
In the library, `pull_request::problems` is new.
