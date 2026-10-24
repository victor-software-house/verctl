---
verctl: minor
---

The reusable release workflow publishes to crates.io through trusted publishing with `crates-io-trusted-publishing: true`, so a caller keeps no registry token. Its publish job now requests `id-token: write`, which every caller grants in the same change that moves its pin, because GitHub checks a called job's permissions when the run starts.
