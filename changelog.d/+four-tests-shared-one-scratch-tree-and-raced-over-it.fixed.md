`dice::scratch` named a directory after what was being tested, not after who asked, so
every test in a binary that asked for the same fixture got the same tree - and started by
emptying it. Cargo runs a binary's tests on parallel threads, so the four tests in
`report.rs` each deleted a tree another was walking. The survey then came back a file
short and the report dropped one of forty callers, which is the assertion that failed.

It read like a wrapping bug in the report, which is the wrong place to look: the emitter
is deterministic and the missing name was a different one each time - `_00` on Linux,
`_39` on Windows - and never missing at all on macOS, which is what losing a race looks
like from the outside. Scheduled CI had been red on this since the tests landed.

A scratch directory is now private per call rather than per name, so no two tests can
ever name the same tree. Nothing about the fixtures or the assertions moved.
