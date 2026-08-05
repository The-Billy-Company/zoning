The first parity pass copied what every sibling repo already had; this one catches what zoning
needed that they didn't, because zoning is the only one of the five that is 100% Rust rather than
Rust bindings over a Zig core. `deny.toml` plus a `cargo deny check` step in the `check` job close
a real gap the siblings did not have either — this crate carries four real dependencies now
(`lsp-server`, `lsp-types`, `serde`, `serde_json`) for the in-process LSP server, and nothing was
watching that graph for a RustSec advisory, a license outside policy, or TLS/async-runtime crates
that transport has no reason to link. `rust-toolchain.toml` resolves `rustfmt`, `clippy`, and the
`wasm32-wasip1` target `editors/zed` needs for a bare-rustup contributor with no mise — pinned to
`stable` rather than a fixed release like the siblings' copy of this file, because every job in
`ci.yml` installs its toolchain with `dtolnay/rust-toolchain@stable`, and a fixed-version pin would
have silently frozen every one of those rolling jobs to whatever release was current the day the
file was written. `.vscode/{settings,extensions,tasks}.json` gives a contributor the same
watcher/search excludes, formatter bindings, and one-click cargo/dogfood/deny/editor tasks the
siblings ship, adapted off Rust rather than Zig as the primary language.
