# Add the Zoning file icon

Associates `.zone` files with Zoning's stacked-parcels mark in Zed's default
icon theme. The SVG is an exact copy of the upstream monochrome asset and uses
`currentColor`, so the default theme controls contrast.

This is intentionally separate from the Zoning language extension. Zed's
publishing policy forbids language extensions from bundling themes or icon
themes.

## Patch

1. Copy `icons/zoning.svg` to
   `assets/icons/file_icons/zoning.svg` in `zed-industries/zed`.
2. Add `"zone"` to the `zoning` entry in the default theme's suffix
   associations.
3. Add the `zoning` icon key and SVG path to the default theme's file icons.
   `association.json` records the intended mapping independent of Rust table
   formatting.
4. Run Zed's icon-theme schema/unit tests and verify both light and dark
   appearances.
