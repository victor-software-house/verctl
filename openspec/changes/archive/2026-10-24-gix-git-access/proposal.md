# Read and write git through gix

## Why

verctl reached git through `git2`, which builds libgit2 with a vendored OpenSSL.
That build needs Perl on every machine that compiles verctl, and it kept the
shared template's `openssl` and `native-tls` bans out of `deny.toml` as a local
edit.

Queue row: VER-050.
A patch: no public type changes, and every command keeps its output.

## What changes

1. Every read moves to `gix` over rustls: HEAD, the merge-base blobs, the
   `added_by` history walk, the index, ignore rules, status, and the one-branch
   fetch `publish` makes.
2. The Version PR commit is written with `gix` in memory, without touching
   HEAD, the index, or the working tree.
3. Push runs `git push`, because gix 0.89 cannot push. The token reaches it
   through a credential helper that reads an environment variable; it is never
   in argv or a URL. The push skips repository hooks, as libgit2 did.
4. Inside a git hook, verctl ignores `GIT_INDEX_FILE`, `GIT_WORK_TREE`, and
   `GIT_DIR`, as libgit2 did, and never writes the index.
5. `deny.toml` takes the template's `openssl` and `native-tls` bans.

## Impact

1. `src/git.rs`, `Cargo.toml`, `deny.toml`, test fixtures (now the `git`
   binary), the instructions, README, and AGENTS.md.
2. `verctl prepare --pr` needs `git` on PATH. GitHub-hosted runners have it.
