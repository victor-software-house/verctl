# verctl

Stack-agnostic Version PR CLI. Fragments are Changesets YAML.
Changelogs are Go templates (chezmoi-style) with the greenfield-release
output contract and `internalAuthors` filtering. Ship
`actions/version-pr` and `actions/publish`. `changelog.ts` is not used.

Operator contract: `verctl instructions` and `skills/verctl/SKILL.md`.
This repo's queue is [`tasks.yaml`](tasks.yaml) (`VER-###`).
