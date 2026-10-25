---
verctl: minor
---

`check --versions` skips in a merge queue run (a `merge_group` event). Each entry's pull request already compared its own versions, and the queue run of the Version PR, or of any entry behind it, carries the new versions, so without the skip the check would fail every release that lands through a queue.

The library's public `versions::Skip` enum gains a `MergeQueue` variant, so a crate that matches on it exhaustively needs a new arm.
