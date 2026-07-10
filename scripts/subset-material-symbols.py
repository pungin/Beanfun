#!/usr/bin/env python3
"""Generate the local Material Symbols Outlined subset used by the app.

The login / QR / region-picker pages render icons through the
`.material-symbols-outlined` font using ligatures (the icon name typed as
text, e.g. `close`, is substituted for the icon glyph). We bundle a subset of
just the icons we use so the ~3.8 MB CDN font becomes a ~10 KB local asset that
works offline.

## Why the two-step approach

`pyftsubset --text="<all icon names>"` over-includes: fonttools' layout closure
keeps every ligature whose component letters are all present, and our icon names
collectively span almost the whole alphabet — so it drags in thousands of
unrelated icons (~2.3 MB). Conversely `--no-layout-closure` drops the ligature
rules entirely, so the icons render as raw letters.

So we (1) strip GSUB down to only the ligatures whose output is an icon we use,
then (2) run a normal subset with closure ON. After step 1 the only ligatures
left are ours, so closure adds exactly those icons.

## Adding a new icon

Add its ligature name to ICONS below and re-run:

    python scripts/subset-material-symbols.py

Requires the dev dependency `material-symbols` (the full font source) plus
Python `fonttools` + `brotli`.
"""

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools import subset

# Every Material Symbols ligature used in a `material-symbols-outlined` span,
# including the dynamic `tile.hintIcon` values (`check_circle`, `info`).
# Keep this list in sync with the templates — a missing name renders as text.
ICONS = [
    "check_circle",
    "close",
    "content_copy",
    "expand_more",
    "flag",
    "info",
    "passkey",
    "qr_code_2",
    "security",
    "tips_and_updates",
    "zoom_in",
]

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "node_modules" / "material-symbols" / "material-symbols-outlined.woff2"
OUT = ROOT / "src" / "assets" / "fonts" / "material-symbols-outlined-subset.woff2"


def prune_gsub_to(font: TTFont, keep: set[str]) -> None:
    """Drop every ligature whose output glyph isn't in `keep`."""
    if "GSUB" not in font:
        return
    for lookup in font["GSUB"].table.LookupList.Lookup:
        for st in lookup.SubTable:
            real = st.ExtSubTable if lookup.LookupType == 7 else st
            if real.__class__.__name__ != "LigatureSubst":
                continue
            for first in list(real.ligatures.keys()):
                kept = [lig for lig in real.ligatures[first] if lig.LigGlyph in keep]
                if kept:
                    real.ligatures[first] = kept
                else:
                    del real.ligatures[first]


def main() -> None:
    if not SRC.exists():
        raise SystemExit(f"missing font source: {SRC}\nrun `npm install` first (material-symbols dev dep)")

    keep = set(ICONS)
    font = TTFont(SRC)
    prune_gsub_to(font, keep)

    options = subset.Options()
    options.flavor = "woff2"
    options.layout_features = ["*"]  # keep liga/calt so ligatures still shape
    options.glyph_names = True  # keep names; negligible size, aids debugging
    options.name_IDs = ["*"]
    options.notdef_outline = True

    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text=" ".join(ICONS))
    subsetter.subset(font)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    font.save(OUT)
    print(f"wrote {OUT.relative_to(ROOT)} ({OUT.stat().st_size / 1024:.1f} KB, {len(font.getGlyphOrder())} glyphs)")


if __name__ == "__main__":
    main()
