The `setup` fake-HOME test named its scratch directory after the current
test-thread name to keep parallel runs from colliding, and Rust spells that
name `setup::tests::fake_home_install_repair_and_uninstall_are_owned` - a
path segment with three colons in it. Unix shrugs; Windows refuses to open a
directory whose name isn't a legal filename, so every `cargo test --release`
on `windows-latest` failed before the real assertions even ran. The thread
name is now sanitized (`:` -> `_`) before it becomes a path component, which
still keeps concurrent test runs apart without asking Windows to accept a
volume-label character in the middle of a directory name.
