## ADDED Requirements

### Requirement: A body within the limit is the changelog

`verctl version --pr` SHALL send the trimmed changelog as the Version PR body when it holds at most 65,536 characters.
It SHALL send the line `Prepared by verctl.` when the changelog is empty.
It SHALL count characters, not bytes.

#### Scenario: A changelog at the limit

- **WHEN** the trimmed changelog holds exactly 65,536 characters, all `é`
- **THEN** the body equals the trimmed changelog byte for byte

#### Scenario: An empty changelog

- **WHEN** the changelog is empty
- **THEN** the body is `Prepared by verctl.`

### Requirement: A longer changelog is cut at a line start

When the trimmed changelog holds more than 65,536 characters, `verctl version --pr` SHALL send the whole lines before the last line start that fits in the budget.
The budget is 65,536 characters minus the closing note and the blank line before it.
The body SHALL hold at most 65,536 characters.

#### Scenario: A changelog of one heading pair and many bullets

- **WHEN** the changelog is `## demo 1.0.1`, `### Patch Changes`, and 2,000 bullets
- **THEN** the body keeps the two headings and every bullet that fits, then ends with the note, within 65,536 characters

#### Scenario: Multibyte text at the cut

- **WHEN** the budget ends inside the text `日本語のメモ 🦀 é ü`
- **THEN** the body keeps whole lines and valid UTF-8, and holds at most 65,536 characters

### Requirement: The cut never lands inside a fenced code block

`verctl version --pr` SHALL NOT end the kept text while a fenced code block is open.
It SHALL treat a run of three or more backticks or tildes as a fence, and a line inside it SHALL NOT start a cut.

#### Scenario: A fence across the budget

- **WHEN** a fenced code block opened by three backticks starts before the budget and closes after it
- **THEN** the kept text ends before the opening fence line

### Requirement: The cut never ends on a heading

When the last kept line is a heading, `verctl version --pr` SHALL drop that heading, and any heading that then ends the kept text, before it adds the note.

#### Scenario: The budget ends just after a heading

- **WHEN** the lines `- one`, a blank line, and `### Late heading` come before a line past the budget
- **THEN** the kept text is `- one`

### Requirement: A cut body ends with a note

A cut body SHALL end with one blank line and then the line `The rest of the changelog is in the changelog files in this pull request's diff.`
When no line fits, the body SHALL be only that line.

#### Scenario: A cut body

- **WHEN** the changelog passes 65,536 characters
- **THEN** the body's last line is `The rest of the changelog is in the changelog files in this pull request's diff.`

#### Scenario: One line past the limit

- **WHEN** the changelog is one line of 65,537 characters
- **THEN** the body is the note alone
