# Design

## 1. The author states the boundary

A pattern is literal text around `{version}`, and `{version}` matches `[0-9]+(\.[0-9]+)*`.
The regex can end a version early, or start one late.
That happens only where the text next to the placeholder is a digit or `.`, or is missing.
So each side of `{version}` needs one of these:

1. a literal character that is neither a digit nor `.`, written in `match`;
2. with `whole_line: true`, the start or the end of the line.

After the placeholder, a `-` or `+` counts only with at least one more literal character after it.
Alone it opens a prerelease or build, and `v{version}-` reads `v1.2.3-rc1` as `1.2.3-`.

Given either, the matched version is the whole run of digits and dots, and a prerelease tail fails the literal text after it.
The arity check then stops the release.
One case stays open: a prerelease that starts with the author's own text, as `download/v{version}-linux` against `download/v1.2.3-linux.1-linux.tar.gz`.
Only the author can close it, and docs/served-files.md section 4 names it.

Alternative considered: refuse when the file's next character continues a version, such as `-` or `+`.
It lost because `download/v1.2.3-linux-x64.tar.gz` is a correct pin whose next character is `-`.

Alternative considered: a new field that names the closing text, such as `ends_with`.
It lost because `match` already holds literal text, and a second place for it splits one spelling in two.

Alternative considered: also accept `.` followed by a non-digit, for a version that ends a sentence.
It lost for now: one rule an author can check by eye beats a rule with an exception.
A sentence-final version moves to its own line, or the sentence names the text after it.

## 2. Failure point

The check runs where every other pattern rule runs: in `Config::load`, as a garde rule on `patterns`.
Every verb that reads `.ctl/ver.yaml` fails before it reads or writes a pinned file.
The message names the id and the fix, for example:
`patterns: install: nothing after {version} marks where the version ends; add the text that follows it to match, or set whole_line: true`.

Alternative considered: fail at rewrite time, only when a file holds a longer version.
It lost because the defect is in the pattern, which a repository can fix before any prerelease reaches a file.
A load-time refusal also fails `check` on the pull request that adds the pattern.

Public override: the boundary written in `match`, or `whole_line: true`.
Disable path: none. An unbounded pattern can half-rewrite a version, and no repository needs one.

## 3. The tool form

A `tool` pin also moves every include of the tool's own repository, at `?ref=v` followed by a version.
That rewrite had no right-hand boundary either.

The ref now runs to the end of the URL, at whitespace, a quote, a comma, a `]`, or the end of the file.

A ref that starts with a digit must be a whole version.
Otherwise the pin fails, naming the include, and nothing is written.
A ref that does not start with a digit, such as a branch, is not a pin and stays put, as before.

Alternative considered: a later row.
It lost because it is the same defect in the same release, and fixing it later would be a second break.

## 4. Callers

| Repository | Pattern | Now | Edit |
|:--|:--|:--|:--|
| qctl | `catalog` | `qctl.git//tasks/q?ref=v{version}` | append `"` |
| qctl | `install` | `github:victor-software-house/qctl@{version}` | append a space and `--` |
| qctl | `skill` | `version: {version}` | add `whole_line: true` |
| forkctl | `catalog` | `forkctl.git//tasks/fork?ref=v{version}` | append `"` |
| forkctl | `skill` | `version: {version}` | add `whole_line: true` |
| rgbctl | `skill` | `version: {version}` | add `whole_line: true` |

forkctl `task`, ctl-core's three `whole_line` patterns, and shrub `internal` pass as they are.
yamled and unixd declare no patterns.
Each edit works on verctl 0.7.2, so callers make it before they take 0.8.0.
