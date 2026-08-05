# Zoning identity

The mark is drawn on a 16 × 16 pixel grid with integer-aligned, filled geometry.
It has no strokes, filters, masks, or subpixel details. `zoning.svg` is the
monochrome master and uses `currentColor`.

## Concepts and selection

- `concepts/stacked-parcels.svg` - three offset parcels; selected because its
  silhouette is distinct at 16 px and directly expresses structural zones.
- `concepts/sealed-doorway.svg` - a guarded boundary with a central seal.
- `concepts/protected-contour.svg` - topographic rings around a protected core.

The doorway reads too much like a letterform, while the contour reads as a
generic target at small sizes. Stacked parcels remains recognizable without
those collisions, so it ships as `zoning.svg`.

## Variants

Use the `currentColor` master wherever the host controls color.
`zoning-light.svg` fixes the mark to `#16191d` for light surfaces;
`zoning-dark.svg` uses `#f3f5f7` for dark surfaces. The concept sources need no
fixed variants because `currentColor` provides the same contrast adaptation.

`contact-sheet.svg` compares every concept at 16, 24, and 32 px on both
backgrounds. Regenerate its PNG with:

```sh
rsvg-convert --width 1440 --height 840 --keep-aspect-ratio \
  --output assets/contact-sheet.png assets/contact-sheet.svg
```

For terminals, use `≡` as the one-cell Unicode fallback and `[=]` where Unicode
width cannot be trusted.
