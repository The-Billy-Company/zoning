//! Arbitrary bytes, in both places bytes arrive from outside.
//!
//! A gate has two mouths: the contract somebody wrote by hand, and the source tree it
//! reads. Both are attacker-adjacent in the only sense that matters for a linter — a
//! half-saved file, a merge conflict marker, a UTF-16 checkout, a source file in a
//! language the dialect only half recognises. None of that is allowed to panic, because
//! a panicking gate is indistinguishable from a broken build and sends its reader
//! looking in the wrong repository.
//!
//! Every case is seeded, and a failure prints the seed. `ZONING_CASES=100000 cargo test
//! --release --test fuzz` is the soak.

#![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

mod dice;

use std::path::{Path, PathBuf};

use dice::Dice;
use zoning::ordinance::Ordinance;
use zoning::survey::{Ask, Survey};

/// The language the fuzzed trees are read in.
fn zig() -> &'static dyn zoning::survey::Dialect {
    zoning::survey::dialect("zig").expect("zig ships in-tree")
}

/// Where the hand-written fixtures live.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// Bend `seed` bytes in `text`, the way a bad save or a lossy checkout would.
fn mangle(rolls: &mut Dice, text: &[u8]) -> Vec<u8> {
    let mut out = text.to_vec();
    for _ in 0..rolls.between(1, 6) {
        if out.is_empty() {
            out.push(b'x');
        }
        let at = rolls.below(out.len());
        match rolls.below(6) {
            0 => out[at] ^= 1 << rolls.below(8),
            1 => out.insert(at, u8::try_from(rolls.below(256)).unwrap_or(b'?')),
            2 => {
                out.remove(at);
            }
            3 => out.truncate(at),
            4 => {
                let slice = out[at..].to_vec();
                out.extend_from_slice(&slice);
            }
            _ => out[at] = *b" \t\n{}\"*<>-#/.".get(rolls.below(13)).unwrap_or(&b' '),
        }
    }
    out
}

/// Read a contract written from `bytes`, and hand back whatever it said.
fn read(dir: &Path, bytes: &[u8]) -> Result<Ordinance, String> {
    let path = dir.join("contract/mutant.zone");
    std::fs::create_dir_all(path.parent().expect("contract dir")).expect("contract dir");
    std::fs::create_dir_all(dir.join("src")).expect("src dir");
    std::fs::write(&path, bytes).expect("write the mutant");
    Ordinance::read(&path, zig()).map_err(|fault| fault.to_string())
}

#[test]
fn the_parser_survives_bytes_that_are_not_a_contract() {
    // No assertion beyond survival is possible here, and none is needed: the harness
    // fails the moment the parser panics, and a `Fault` for nonsense is the correct
    // answer to nonsense. What this pins is that *every* answer is one of the two.
    let scratch = dice::scratch("fuzz-noise");
    let mut rolls = Dice::new(dice::seed());
    let alphabet = b"package zones{} \n\tseal keep use limit forbid variance because reach hops \
                     root facade language exclude to through by across cycles nobody -> \"*\"/.#";
    for index in 0..dice::cases(4000) {
        let len = rolls.between(0, 240);
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                if rolls.odds(3) {
                    u8::try_from(rolls.below(256)).unwrap_or(b'?')
                } else {
                    alphabet[rolls.below(alphabet.len())]
                }
            })
            .collect();
        let dir = scratch.join(format!("case-{index}"));
        if let Err(fault) = read(&dir, &bytes) {
            assert!(
                !fault.trim().is_empty(),
                "a fault has to say something (case {index}): {bytes:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_mangled_contract_either_parses_or_locates_its_fault() {
    // Every fixture is a valid contract, so mutating one lands near the grammar rather
    // than in noise — the region where a parser's offset arithmetic actually lives. The
    // invariant is the promise the error format makes: a fault names the file it is
    // about. A fault that cannot say where it happened costs its reader the whole search.
    let originals: Vec<Vec<u8>> =
        zoning::ordinance::discover(&fixtures(), &["pass".into(), "fail".into()])
            .iter()
            .filter_map(|path| std::fs::read(path).ok())
            .collect();
    assert!(!originals.is_empty(), "no fixture contracts to mangle — did the tree move?");

    let scratch = dice::scratch("fuzz-mangle");
    let mut broken = Vec::new();
    for index in 0..dice::cases(2000) {
        let seed = dice::seed() ^ (index as u64).wrapping_mul(0x1000_0001B3);
        let mut rolls = Dice::new(seed);
        let source = &originals[rolls.below(originals.len())];
        let bytes = mangle(&mut rolls, source);
        let dir = scratch.join(format!("case-{index}"));
        if let Err(fault) = read(&dir, &bytes) {
            let readable = std::str::from_utf8(&bytes).is_ok();
            if readable && !fault.contains("mutant.zone") {
                broken.push(format!("  ZONING_SEED={seed} — unlocated fault: {fault}"));
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(broken.is_empty(), "{} case(s) failed:\n{}", broken.len(), broken.join("\n"));
}

#[test]
fn a_contract_that_survives_mangling_can_still_be_judged() {
    // Parsing is only half the exposure. A mutation that stays *legal* — a glob that
    // matches nothing, a zone whose paths overlap another's, a seal whose door is not in
    // the region it seals, a reach ceiling of zero — reaches the laws, and the laws index
    // and slice paths. This is the half a grammar fuzzer never gets to.
    let layered = fixtures().join("pass/layered");
    let original = std::fs::read(layered.join("contract/layered.zone")).expect("the fixture");
    let scratch = dice::scratch("fuzz-judge");
    let mut judged = 0_usize;
    for index in 0..dice::cases(2000) {
        let mut rolls = Dice::new(dice::seed() ^ (index as u64).wrapping_mul(0x100_0193));
        let bytes = mangle(&mut rolls, &original);
        let dir = scratch.join(format!("case-{index}"));
        if let Ok(ordinance) = read(&dir, &bytes) {
            // Judged against the real tree, not the empty scratch one: a contract that
            // matches nothing exercises none of the laws.
            let survey = Survey::of(&Ask {
                repo_root: &layered,
                module_root: &layered.join("src"),
                exclude: &ordinance.exclude,
                dialect: ordinance.dialect,
                tracked: None,
            });
            let _ = zoning::judge::judge(&survey, &ordinance);
            judged += 1;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(judged > 0, "no mutation stayed legal — the mutator is too destructive to be a test");
}

#[test]
fn the_import_scanner_survives_source_that_is_not_source() {
    // The dialect walks bytes looking for import specs and remembers offsets into them,
    // which is where the last real panic in this tool lived. Random bytes in a file the
    // survey will read is the direct test of it, and an unterminated string or a spec
    // that is pure high bytes is the case a hand-written fixture never thinks of.
    let scratch = dice::scratch("fuzz-source");
    let mut rolls = Dice::new(dice::seed());
    let bait = [
        &b"@import(\""[..],
        &b"\")"[..],
        &b"//"[..],
        &b"\\\\"[..],
        &b"../"[..],
        &b".zig"[..],
        &b"\""[..],
        &b"pub const "[..],
    ];
    for index in 0..dice::cases(600) {
        let dir = scratch.join(format!("case-{index}"));
        std::fs::create_dir_all(dir.join("src")).expect("src dir");
        for file in 0..rolls.between(1, 3) {
            let mut bytes = Vec::new();
            for _ in 0..rolls.between(1, 40) {
                if rolls.odds(2) {
                    bytes.extend_from_slice(bait[rolls.below(bait.len())]);
                } else {
                    bytes.push(u8::try_from(rolls.below(256)).unwrap_or(b'?'));
                }
            }
            std::fs::write(dir.join(format!("src/f{file}.zig")), &bytes).expect("write source");
        }
        let module = dir.join("src");
        let _ = Survey::of(&Ask {
            repo_root: &dir,
            module_root: &module,
            exclude: &[],
            dialect: zig(),
            tracked: None,
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
