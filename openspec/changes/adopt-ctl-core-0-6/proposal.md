# Adopt the ctl-core 0.6 presentation defaults

## Why

verctl pins ctl-core 0.5.0. ctl-core 0.6.2 ships the look chosen in its
`choose-visual-identity` change: borderless records, an identifier role, pretty
JSON, a two-column automatic-width buffer, and an 80-column fallback when no
width is detected. On 0.5.0 a two-line `publish --dry-run` summary prints a
four-line box, and piped output has no width limit at all.

Queue row: VER-040.

## What changes

1. Pin ctl-core `=0.6.2`.
2. Package names render through the identifier role (`Table::id_column`).
   Build targets and check names stay tokens.
3. Tests expect the borderless records, and JSON tests compare parsed values
   rather than one-line text.

## Impact

Pretty output changes shape: records lose their box and JSON is indented.
The JSON data itself does not change.
