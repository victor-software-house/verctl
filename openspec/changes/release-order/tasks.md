# Tasks

## 1. Release

- [ ] 1.1 Order: draft and assets, registries, then the draft published with its tag. Proof: a CI test that records the calls in order.
- [ ] 1.2 A partial release is a `status` state with its cause. Proof: a snapshot test of the human and JSON views.
- [ ] 1.3 A rerun skips every crate the index lists. Proof: a CI test against a recorded index with one of three crates live.

## 2. Documents

- [ ] 2.1 Update AGENTS.md, the instructions, the operator skill and the README for the order and the partial state. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
