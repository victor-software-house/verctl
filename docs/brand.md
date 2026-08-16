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
| Rust | `#c45c2a` | The new version. One accent, never two |

Banner-only tints: `#6f675c` (muted on cream), `#8d857a` (muted on ink),
`#ddd6c8` / `#2f2f2f` (hairline).

## Construction

A 32-unit square, corner radius 6. Line from `4,28` to `26,6` at 2 units
wide; nodes on that line at `6.5,25.5` and `15.25,16.75` at r 3.5; the new
tag at `24,8` at r 5.75. Every node centre satisfies `x + y = 32`, so the
line needs no separate alignment. It is checked at 16px — keep the node
gaps, they are what stop it reading as one blob.

## Banner text

Set in [Geist Mono](https://github.com/vercel/geist-font) (OFL) and converted
to outlines, so nothing depends on a font at render time: wordmark Black 60px
with −3 tracking, tagline Medium 17px, chip Regular 16px. To change the
wording, reshape with `fonttools` + `uharfbuzz` at those sizes rather than
adding a `<text>` element.
