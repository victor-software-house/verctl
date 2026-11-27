---
verctl: patch!
---

verctl builds on ctl-core 0.8.0-rc.2, and every command takes its arguments from an input document.
`--input FILE`, or `--input -` for stdin, and `--input-format NAME` fill the selected command from a YAML or JSON document, and `VERCTL_INPUT` and `VERCTL_INPUT_FORMAT` set them.
The keys of the document are long option names and positional ids.
The command line beats the environment, the environment beats the document, and the document beats the default, and a key that loses is dropped.
An unknown key fails the run with exit code 2, naming the file, the line, and the key.

`--json` (`VERCTL_JSON`) is a shorthand for `--format json`, and `--stream` (`VERCTL_STREAM`) writes each record as the command produces it.

Output that a script reads changes with ctl-core 0.8:

1. Piped output is `tsv`, where it was the pretty table. `--format pretty` or `VERCTL_FORMAT=pretty` gives the table.
2. A JSON failure goes to stderr and stdout stays empty. A caller that read the error from stdout reads stderr.
3. A parse error reads `verctl: <message>`, without the `error:` prefix.
4. A handler error's JSON `message` is its outermost error, and the rest of its chain is in `causes`, where `message` joined it with `: `.
5. A table never narrows a column below its floor, and a table that cannot fit at its floors stacks each row as a record.
6. `--help` lists `--json` and `--stream`, and `--format` no longer shows `[default: pretty]`.
7. A declared file that holds a sequence where a mapping belongs reports `expected a mapping, found a sequence`.
