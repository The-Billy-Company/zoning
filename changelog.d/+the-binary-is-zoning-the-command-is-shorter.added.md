The crate is `zoning` and always will be — that's the thing you `cargo install`
or `pip install` — but nobody wants to type six syllables before every `verify`.
`zone` is now installed alongside it, from the same source, as the second
`[[bin]]` target of the identical binary: `cargo install zoning`, `pipx install
zoning`, and `uv tool install zoning` all put both `zone` and `zoning` on
`PATH`, and either name runs the same executable byte-for-byte apart from its
own filename.

`zone` is the one the docs teach now — `--help`, `--version`, every error
message, and the `zone map`/`zone [package]:` report headers all say `zone`
regardless of which name launched them, the way `rg --version` says `rg` and
not `ripgrep`. `zoning` keeps working exactly as before for anyone whose
fingers, scripts, or CI YAML already know it; nothing that names `zoning`
today needs to change.
