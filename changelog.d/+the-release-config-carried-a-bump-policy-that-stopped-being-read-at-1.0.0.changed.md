Release versioning is documented where you pick a commit prefix, and the two
settings that made it look like something other than semver are gone.

`bump-minor-pre-major` and `bump-patch-for-minor-pre-major` sat in
`release-please-config.json` since before 1.0.0. release-please reads them only
while the version is below 1.0.0, so they have meant nothing since 1.0.0 while
still reading like a bump policy - and this repo had never cut a patch release,
which made them look like the reason. They were not: every release window so
far happened to carry exactly one `feat`.

What actually decides the number is now a table in `CONTRIBUTING.md` - `!` or a
BREAKING CHANGE footer takes the major, `feat` takes the minor, everything else
takes the patch - along with the `Release-As: X.Y.Z` footer, which pins an exact
version when the rules would not pick it and which was previously written down
nowhere.
