---
verctl: patch
---

The release binary no longer carries octocrab's unused JWT crypto, which RUSTSEC-2026-0387 flagged in `crypto-bigint`. verctl authenticates with a personal token only.
