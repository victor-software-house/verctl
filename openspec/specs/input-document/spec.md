# input-document Specification

## Purpose

How a verctl command takes its arguments from a YAML or JSON document through `--input`, and which source wins when the command line, the environment and the document all set one.

## Requirements

### Requirement: A command reads its arguments from a document

Every verctl command SHALL take `--input <FILE>` and `--input-format <NAME>`, also read from `VERCTL_INPUT` and `VERCTL_INPUT_FORMAT`.
A document's keys SHALL be long option names and positional ids, and each value SHALL pass the argument's own parser.
The command line SHALL win over the environment, the environment over the document, and the document over the default.

#### Scenario: Status from a JSON document

- **WHEN** `verctl status --input doc.json --format json` runs where `doc.json` holds `{"dir": "other/.changeset"}`
- **THEN** it writes the same report as `verctl status --dir other/.changeset --format json`

#### Scenario: An unknown key

- **WHEN** `doc.yaml` holds `dirr: other` and `verctl status --input doc.yaml` runs
- **THEN** it exits 2, and the error names `doc.yaml`, line 1 and the key `dirr`
