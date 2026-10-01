---
verctl: patch
---

`prepare` and `check` refuse a `minor` or `major` fragment on a package at exactly `0.0.0`, so a new package releases `0.0.1` first. Setting `first_minor: true` on a package in `.ctl/ver.yaml` lets the operator open it at `0.1.0`. `check` now plans the pending fragments against `.ctl/ver.yaml`, so it also fails on a package the declarations do not name.
