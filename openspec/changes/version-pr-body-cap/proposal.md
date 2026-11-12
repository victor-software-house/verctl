# Cap the Version PR body at GitHub's limit

## Motivation

GitHub rejects a pull request body over 65,536 characters.
`release::pr_body` sends the whole changelog as the Version PR body ([`src/release.rs`][release]).
A repository with many pending fragments passes the limit, and then the Version PR cannot be created or updated.

nishiki had 275 pending fragments.
Its Release run failed while it opened the Version PR, with "body is too long (maximum is 65536 characters)" ([run 43146966330][nishiki-run]).
That repository cannot release until the body fits.

Queue row: [VER-126][ver-126].

## Changes

1. `pr_body` returns the trimmed changelog as it was when it holds at most 65,536 characters.
2. A longer changelog is cut at a line start, so the body keeps every whole line that fits.
3. The cut never lands inside a fenced code block.
   A heading left as the last kept line is dropped, so the body never ends with an empty heading.
4. The cut body ends with one line that points at the changelog files in the pull request's diff.
5. `release::PR_BODY_LIMIT` holds the limit, and the budget for the changelog leaves room for the note.
6. The instructions state the cut.

## Capabilities

### New Capabilities

- `version-pr-body`: what the Version PR body holds, within and past GitHub's limit.

### Modified Capabilities

None.

## Impact

1. Code: `src/release.rs`, `tests/release.rs`.
2. Bundle: `src/instructions.md` and its template state the cut.
   The operator skill states no body rule, so it does not change.
3. Callers: none must change.
   A changelog under the limit gives the same body as before.
4. Release: a patch.
   `release::PR_BODY_LIMIT` is a new public constant.

[nishiki-run]: https://github.com/victor-software-house/nishiki/actions/runs/43146966330
[release]: ../../../src/release.rs
[ver-126]: ../../../tasks.yaml
