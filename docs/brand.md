# Mark

The mark is a release history: two shipped versions on a rising line, the
newest tag larger and in rust. It is the only mark; earlier diamond, bar, and
tag sketches are dead.

| File | Use |
|:--|:--|
| [`mark.svg`](mark.svg) | Square mark, cream field. Avatar, favicon, docs. |
| [`mark-dark.svg`](mark-dark.svg) | Same geometry, ink field. |
| [`banner.svg`](banner.svg) | README header, 1200×240. |
| [`banner-dark.svg`](banner-dark.svg) | Same, ink field. |

Pair the two fields with `<picture>` and `prefers-color-scheme`, as the README
header does. Never recolour a single file at the call site.

## Palette

| | Hex | Role |
|:--|:--|:--|
| Cream | `#f3efe6` | Field, or figure on ink |
| Ink | `#161616` | Figure, or field |
| Rust | `#c45c2a` | The new version, and the same accent on the banner chip |

The square mark has one rust shape. The banner repeats that rust on the
chip so the header matches the mark. It is the same accent, not a second
role.

Banner-only tints: `#6f675c` (muted on cream), `#8d857a` (muted on ink),
`#ddd6c8` / `#2f2f2f` (hairline).

## Construction

A 32-unit square, corner radius 6. Line from `7.5,24.5` to `22.5,9.5` at 2
units wide; nodes on that line at `7.5,24.5` and `15,17` at r 3; the new tag
at `22.5,9.5` at r 4.75. Every node centre satisfies `x + y = 32`, so the line
needs no separate alignment, and the three sit 7.5 units apart in `x`. The
figure clears the field by 4.5 units at the first node and 4.75 at the tag.

Pad by moving the geometry, never by scaling the figure inside the square: a
scale takes the 2-unit line and the node radii down with it, so the mark loses
weight exactly where it is already thinnest, and it shows first at 16px — which
is the size it is checked at. Keep the node gaps for the same reason; they are
what stop it reading as one blob.

## Banner text

Set in [Geist Mono](https://github.com/vercel/geist-font) (OFL) and converted
to outlines, so nothing depends on a font at render time: wordmark Black 60px
with −3 tracking, tagline Medium 17px, chip Regular 16px. To change the
wording, reshape with `fonttools` + `uharfbuzz` at those sizes rather than
adding a `<text>` element.
