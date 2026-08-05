//! Python — the second dialect, and the one whose imports are not paths.
//!
//! Every other property this tool needs falls out of one difference from Zig:
//! `@import("../a/b.zig")` already spells the file to open, but `import a.b` and
//! `from . import c` spell a *module name* — dotted, sometimes anchored to the
//! importing file by a run of leading dots, resolved against `sys.path` by an
//! algorithm this tool does not run. So this dialect's whole job is arithmetic
//! that [`super::resolve`] never has to see: turn a dotted or dot-relative spelling
//! into the same `../`-collapsed path a Zig import would have written, using only
//! the importing file's own depth and the set of top-level names the survey already
//! knows are local — never a filesystem probe, and never PYTHONPATH.
//!
//! A dotted or absolute spelling is ambiguous in one more way a path is not: `x.py`
//! and `x/__init__.py` are both legal answers to "the file named `x`", and nothing
//! in the import statement says which. Both are offered as candidates; at most one
//! can exist in a real tree, and [`super::resolve`]'s caller already drops whichever
//! one is not a judged file, so an author never sees the ambiguity.

use super::dialect::{Dialect, Import};
use super::prose::Prose;

/// The Python dialect.
pub(super) struct Python;

// TOML and Python happen to agree on prose: `#` to end of line, `"`/`'` single-line
// literals, `"""`/`'''` literals that may hold a raw newline. One table serves both
// `.py` source and `pyproject.toml`.
const PROSE: Prose = Prose {
    line_comment: "#",
    block_comment: None,
    line_string: None,
    quotes: b"\"'",
    triple_quotes: &["\"\"\"", "'''"],
};

impl Dialect for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn manifests(&self) -> &'static [&'static str] {
        &["pyproject.toml", "setup.py", "setup.cfg"]
    }

    // `path = "…"` is how every tool that admits an in-tree dependency spells it —
    // uv and Poetry both write it as a key inside an inline table under
    // `[tool.*.sources]`/`[tool.*.dependencies]`. The key alone recovers it without a
    // TOML parser, same trade Zig's `.path` search makes: a stray match only ever
    // excuses a directory the manifest already pointed at.
    fn vendored(&self, manifest: &str) -> Vec<String> {
        let code = PROSE.code_only(manifest);
        let raw = manifest.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(head) = find_word(&code, b"path", i) {
            let mut j = head + 4;
            while matches!(code.get(j), Some(b' ' | b'\t')) {
                j += 1;
            }
            i = head + 1;
            if code.get(j) != Some(&b'=') {
                continue;
            }
            if let Some(dir) = literal(raw, j + 1) {
                out.push(dir.trim_end_matches('/').to_owned());
            }
        }
        out
    }

    // `name` under the standard `[project]` table (PEP 621). `setup.py`/`setup.cfg`
    // name a package no more uniformly than they format the rest of themselves, so
    // this reads only the one manifest shape that actually declares it in one place.
    fn declared(&self, manifest: &str) -> Option<String> {
        let code = PROSE.code_only(manifest);
        let raw = manifest.as_bytes();
        let head = find_line(&code, b"[project]")?;
        let body_start = head + b"[project]".len();
        let body_end = find_next_table(&code, body_start);
        let mut i = body_start;
        while i < body_end {
            let line_end =
                code[i..body_end].iter().position(|&b| b == b'\n').map_or(body_end, |o| i + o);
            let indent =
                code[i..line_end].iter().take_while(|&&b| matches!(b, b' ' | b'\t')).count();
            let key = i + indent;
            if code[key..line_end].starts_with(b"name") {
                let mut j = key + 4;
                while matches!(code.get(j), Some(b' ' | b'\t')) {
                    j += 1;
                }
                if code.get(j) == Some(&b'=')
                    && let Some(value) = literal(raw, j + 1)
                {
                    return Some(value);
                }
            }
            i = line_end + 1;
        }
        None
    }

    // The union of `sys.stdlib_module_names` across every CPython from 3.10 (the
    // first release to expose the set at all) through 3.14, so a contract reads the
    // same regardless of which interpreter actually runs it. A name retired after
    // 3.9 or not yet added is still ambient here — over-including the standard
    // library costs nothing a contract would notice, where under-including it would
    // demand a `use` grant for `cgi` on one version and not another.
    fn ambient(&self) -> &'static [&'static str] {
        STDLIB
    }

    fn prose(&self) -> &Prose {
        &PROSE
    }

    fn imports(&self, path: &str, roots: &[&str], _source: &str, code: &[u8]) -> Vec<Import> {
        let depth = depth_of(path);
        let mut out = Vec::new();
        let mut i = 0;
        while i < code.len() {
            let from_at = find_word(code, b"from", i);
            let import_at = find_word(code, b"import", i);
            let Some(at) = min_of(from_at, import_at) else { break };
            if !at_statement_start(code, at) {
                i = at + 1;
                continue;
            }
            if from_at == Some(at) {
                let (found, end) = parse_from(code, roots, depth, at);
                out.extend(found);
                i = end;
            } else {
                let (found, end) = parse_plain(code, roots, depth, at);
                out.extend(found);
                i = end;
            }
        }
        out
    }

    // A local candidate is always spelled as a path this dialect itself produced in
    // `imports` — every other spec is the bare top-level name of a module the build
    // (or the standard library) resolves, never a path.
    #[allow(clippy::case_sensitive_file_extension_comparisons, reason = "import specs are literal")]
    fn is_local(&self, spec: &str) -> bool {
        spec.ends_with(".py")
    }

    fn escape_remedy(&self) -> &'static str {
        "Python cannot follow a relative import past the top of its own package; \
         name the dependency as an absolute import instead"
    }
}

/// `from <dots><module>? import <names>` — `at` is the byte offset of `from`.
/// Returns the imports it read and the offset just past the statement.
fn parse_from(code: &[u8], roots: &[&str], depth: usize, at: usize) -> (Vec<Import>, usize) {
    let mut j = at + 4;
    if !matches!(code.get(j), Some(b' ' | b'\t')) {
        return (Vec::new(), at + 4);
    }
    j = skip_h(code, j);
    let dots = code[j..].iter().take_while(|&&b| b == b'.').count();
    j += dots;
    let mod_start = j;
    j = skip_dotted(code, j);
    let module = std::str::from_utf8(&code[mod_start..j]).unwrap_or("");
    j = skip_h(code, j);
    let Some(k) = find_word(code, b"import", j) else { return (Vec::new(), at + 4) };
    if code[j..k].iter().any(|&b| !matches!(b, b' ' | b'\t' | b'\n')) {
        return (Vec::new(), at + 4); // `from` used elsewhere, e.g. `raise x from y`
    }
    j = skip_h(code, k + 6);
    let (names, end) = read_name_list(code, j);

    let mut out = Vec::new();
    if dots == 0 {
        let top = module.split('.').next().unwrap_or("");
        if top.is_empty() || !roots.contains(&top) {
            if !top.is_empty() {
                out.push(spec(at, top.to_owned()));
            }
            return (out, end);
        }
    }
    let climbs = if dots == 0 { depth } else { dots - 1 };
    let prefix = "../".repeat(climbs);
    let dir = if module.is_empty() {
        prefix.trim_end_matches('/').to_owned()
    } else {
        format!("{prefix}{}", module.replace('.', "/"))
    };
    if !module.is_empty() {
        out.push(spec(at, format!("{dir}.py")));
    }
    out.push(spec(at, format!("{dir}/__init__.py")));
    for name in names {
        if name == "*" {
            continue;
        }
        out.push(spec(at, format!("{dir}/{name}.py")));
        out.push(spec(at, format!("{dir}/{name}/__init__.py")));
    }
    (out, end)
}

/// `import <dotted>[ as alias][, <dotted>[ as alias]]*` — `at` is the byte offset of
/// `import`. Returns the imports it read and the offset just past the statement.
fn parse_plain(code: &[u8], roots: &[&str], depth: usize, at: usize) -> (Vec<Import>, usize) {
    let start = at + 6;
    let eol = code[start..]
        .iter()
        .position(|&b| b == b'\n' || b == b';')
        .map_or(code.len(), |o| start + o);
    let mut out = Vec::new();
    for piece in code[start..eol].split(|&b| b == b',') {
        let piece = trim(piece);
        if piece.is_empty() {
            continue;
        }
        let cut = find(piece, b" as ").unwrap_or(piece.len());
        let Ok(dotted) = std::str::from_utf8(trim(&piece[..cut])) else { continue };
        if dotted.is_empty() {
            continue;
        }
        let top = dotted.split('.').next().unwrap_or("");
        if !roots.contains(&top) {
            out.push(spec(at, top.to_owned()));
            continue;
        }
        let target = format!("{}{}", "../".repeat(depth), dotted.replace('.', "/"));
        out.push(spec(at, format!("{target}.py")));
        out.push(spec(at, format!("{target}/__init__.py")));
    }
    (out, eol)
}

fn spec(offset: usize, spec: String) -> Import {
    Import { offset, spec }
}

/// How many directories separate `path` from the module root.
fn depth_of(path: &str) -> usize {
    path.rsplit_once('/').map_or(0, |(dir, _)| dir.split('/').count())
}

/// Byte offset of whichever of two candidate positions is earlier.
fn min_of(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

/// Is the byte at `i`, after only horizontal whitespace, the start of a statement?
fn at_statement_start(code: &[u8], i: usize) -> bool {
    let mut j = i;
    while j > 0 && matches!(code[j - 1], b' ' | b'\t') {
        j -= 1;
    }
    j == 0 || matches!(code[j - 1], b'\n' | b';' | b':')
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Next word-bounded occurrence of `word` at or after `from`.
fn find_word(code: &[u8], word: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while let Some(k) = find(&code[i..], word) {
        let at = i + k;
        let before = at == 0 || !is_ident(code[at - 1]);
        let after = code.get(at + word.len()).is_none_or(|&b| !is_ident(b));
        if before && after {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// Skip spaces and tabs only — never a newline, since a bare header token never
/// legitimately continues onto the next line without parens or a backslash.
fn skip_h(code: &[u8], mut j: usize) -> usize {
    while matches!(code.get(j), Some(b' ' | b'\t')) {
        j += 1;
    }
    j
}

/// Skip an `identifier(.identifier)*` run, approximating PEP 3131: any byte ≥ 0x80 is
/// assumed to continue a Unicode identifier rather than lexing one exactly.
fn skip_dotted(code: &[u8], mut j: usize) -> usize {
    while code
        .get(j)
        .is_some_and(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.') || b >= 0x80)
    {
        j += 1;
    }
    j
}

/// The `import` clause's name list: parenthesized (may span lines) or a bare
/// comma list running to end of line. Returns the names and the offset just past.
fn read_name_list(code: &[u8], j: usize) -> (Vec<String>, usize) {
    if code.get(j) == Some(&b'(') {
        let close = find(&code[j + 1..], b")").map_or(code.len(), |o| j + 1 + o);
        (split_names(&code[j + 1..close]), (close + 1).min(code.len()))
    } else {
        let eol =
            code[j..].iter().position(|&b| b == b'\n' || b == b';').map_or(code.len(), |o| j + o);
        (split_names(&code[j..eol]), eol)
    }
}

fn split_names(chunk: &[u8]) -> Vec<String> {
    chunk
        .split(|&b| b == b',')
        .filter_map(|raw| {
            let piece = trim(raw);
            if piece.is_empty() {
                None
            } else if piece == b"*" {
                Some("*".to_owned())
            } else {
                let cut = find(piece, b" as ").unwrap_or(piece.len());
                std::str::from_utf8(trim(&piece[..cut]))
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            }
        })
        .collect()
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|&b| !b.is_ascii_whitespace()).map_or(start, |p| p + 1);
    &bytes[start..end]
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Byte offset of a line whose entire trimmed content is exactly `needle`.
fn find_line(code: &[u8], needle: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < code.len() {
        let end = code[i..].iter().position(|&b| b == b'\n').map_or(code.len(), |o| i + o);
        if trim(&code[i..end]) == needle {
            return Some(i);
        }
        i = end + 1;
    }
    None
}

/// The next line starting with `[` at or after `from`, or the end of the buffer —
/// the boundary of the table `from` sits inside.
fn find_next_table(code: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < code.len() {
        let end = code[i..].iter().position(|&b| b == b'\n').map_or(code.len(), |o| i + o);
        if trim(&code[i..end]).first() == Some(&b'[') {
            return i;
        }
        i = end + 1;
    }
    code.len()
}

/// Read the TOML string literal opening at or after `from` — basic (`"…"`, with
/// backslash escapes) or literal (`'…'`, without).
fn literal(raw: &[u8], from: usize) -> Option<String> {
    let mut i = from;
    while raw.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    let quote = *raw.get(i)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    while let Some(&ch) = raw.get(i) {
        match ch {
            b'\\' if quote == b'"' && i + 1 < raw.len() => {
                out.push(raw[i + 1]);
                i += 2;
            }
            b'\n' => return None,
            b if b == quote => return String::from_utf8(out).ok(),
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    None
}

/// The union of `sys.stdlib_module_names` across `CPython` 3.10 through 3.14.
#[rustfmt::skip]
const STDLIB: &[&str] = &[
    "__future__", "_abc", "_aix_support", "_android_support", "_apple_support", "_ast",
    "_ast_unparse", "_asyncio", "_bisect", "_blake2", "_bootsubprocess", "_bz2", "_codecs",
    "_codecs_cn", "_codecs_hk", "_codecs_iso2022", "_codecs_jp", "_codecs_kr", "_codecs_tw",
    "_collections", "_collections_abc", "_colorize", "_compat_pickle", "_compression",
    "_contextvars", "_crypt", "_csv", "_ctypes", "_curses", "_curses_panel", "_datetime",
    "_dbm", "_decimal", "_elementtree", "_frozen_importlib", "_frozen_importlib_external",
    "_functools", "_gdbm", "_hashlib", "_heapq", "_hmac", "_imp", "_interpchannels",
    "_interpqueues", "_interpreters", "_io", "_ios_support", "_json", "_locale", "_lsprof",
    "_lzma", "_markupbase", "_md5", "_msi", "_multibytecodec", "_multiprocessing", "_opcode",
    "_opcode_metadata", "_operator", "_osx_support", "_overlapped", "_pickle", "_posixshmem",
    "_posixsubprocess", "_py_abc", "_py_warnings", "_pydatetime", "_pydecimal", "_pyio",
    "_pylong", "_pyrepl", "_queue", "_random", "_remote_debugging", "_scproxy", "_sha1",
    "_sha2", "_sha256", "_sha3", "_sha512", "_signal", "_sitebuiltins", "_socket", "_sqlite3",
    "_sre", "_ssl", "_stat", "_statistics", "_string", "_strptime", "_struct", "_suggestions",
    "_symtable", "_sysconfig", "_thread", "_threading_local", "_tkinter", "_tokenize",
    "_tracemalloc", "_types", "_typing", "_uuid", "_warnings", "_weakref", "_weakrefset",
    "_winapi", "_wmi", "_zoneinfo", "_zstd", "abc", "aifc", "annotationlib", "antigravity",
    "argparse", "array", "ast", "asynchat", "asyncio", "asyncore", "atexit", "audioop",
    "base64", "bdb", "binascii", "binhex", "bisect", "builtins", "bz2", "cProfile", "calendar",
    "cgi", "cgitb", "chunk", "cmath", "cmd", "code", "codecs", "codeop", "collections",
    "colorsys", "compileall", "compression", "concurrent", "configparser", "contextlib",
    "contextvars", "copy", "copyreg", "crypt", "csv", "ctypes", "curses", "dataclasses",
    "datetime", "dbm", "decimal", "difflib", "dis", "distutils", "doctest", "email",
    "encodings", "ensurepip", "enum", "errno", "faulthandler", "fcntl", "filecmp", "fileinput",
    "fnmatch", "fractions", "ftplib", "functools", "gc", "genericpath", "getopt", "getpass",
    "gettext", "glob", "graphlib", "grp", "gzip", "hashlib", "heapq", "hmac", "html", "http",
    "idlelib", "imaplib", "imghdr", "imp", "importlib", "inspect", "io", "ipaddress",
    "itertools", "json", "keyword", "lib2to3", "linecache", "locale", "logging", "lzma",
    "mailbox", "mailcap", "marshal", "math", "mimetypes", "mmap", "modulefinder", "msilib",
    "msvcrt", "multiprocessing", "netrc", "nis", "nntplib", "nt", "ntpath", "nturl2path",
    "numbers", "opcode", "operator", "optparse", "os", "ossaudiodev", "pathlib", "pdb",
    "pickle", "pickletools", "pipes", "pkgutil", "platform", "plistlib", "poplib", "posix",
    "posixpath", "pprint", "profile", "pstats", "pty", "pwd", "py_compile", "pyclbr", "pydoc",
    "pydoc_data", "pyexpat", "queue", "quopri", "random", "re", "readline", "reprlib",
    "resource", "rlcompleter", "runpy", "sched", "secrets", "select", "selectors", "shelve",
    "shlex", "shutil", "signal", "site", "smtpd", "smtplib", "sndhdr", "socket",
    "socketserver", "spwd", "sqlite3", "sre_compile", "sre_constants", "sre_parse", "ssl",
    "stat", "statistics", "string", "stringprep", "struct", "subprocess", "sunau", "symtable",
    "sys", "sysconfig", "syslog", "tabnanny", "tarfile", "telnetlib", "tempfile", "termios",
    "textwrap", "this", "threading", "time", "timeit", "tkinter", "token", "tokenize",
    "tomllib", "trace", "traceback", "tracemalloc", "tty", "turtle", "turtledemo", "types",
    "typing", "unicodedata", "unittest", "urllib", "uu", "uuid", "venv", "warnings", "wave",
    "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib", "xml", "xmlrpc",
    "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
];

#[cfg(test)]
#[allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]
mod tests {
    use super::*;

    fn specs(path: &str, roots: &[&str], source: &str) -> Vec<String> {
        let code = PROSE.code_only(source);
        Python.imports(path, roots, source, &code).into_iter().map(|i| i.spec).collect()
    }

    #[test]
    fn plain_absolute_import_of_a_root_package() {
        assert_eq!(
            specs("pkg/mod.py", &["pkg"], "import pkg.util\n"),
            ["../pkg/util.py", "../pkg/util/__init__.py"]
        );
    }

    #[test]
    fn plain_import_of_an_external_dependency() {
        assert_eq!(specs("pkg/mod.py", &["pkg"], "import numpy as np\n"), ["numpy"]);
    }

    #[test]
    fn comma_separated_plain_imports() {
        // The importer is loose at the module root, so no climb is needed to reach a
        // sibling root package.
        assert_eq!(
            specs("mod.py", &["pkg"], "import os, pkg.a, sys\n"),
            ["os", "pkg/a.py", "pkg/a/__init__.py", "sys"]
        );
    }

    #[test]
    fn from_import_of_named_submodules() {
        assert_eq!(
            specs("pkg/mod.py", &["pkg"], "from pkg import a, b\n"),
            [
                "../pkg.py",
                "../pkg/__init__.py",
                "../pkg/a.py",
                "../pkg/a/__init__.py",
                "../pkg/b.py",
                "../pkg/b/__init__.py",
            ]
        );
    }

    #[test]
    fn from_import_of_an_external_module_is_one_departure() {
        assert_eq!(
            specs("pkg/mod.py", &["pkg"], "from collections import OrderedDict\n"),
            ["collections"]
        );
    }

    #[test]
    fn relative_import_same_directory() {
        assert_eq!(
            specs("pkg/mod.py", &["pkg"], "from . import sibling\n"),
            ["/__init__.py", "/sibling.py", "/sibling/__init__.py"]
        );
    }

    #[test]
    fn relative_import_climbs_a_level() {
        assert_eq!(
            specs("pkg/sub/mod.py", &["pkg"], "from .. import sibling\n"),
            ["../__init__.py", "../sibling.py", "../sibling/__init__.py"]
        );
    }

    #[test]
    fn relative_import_with_a_named_module() {
        assert_eq!(
            specs("pkg/sub/mod.py", &["pkg"], "from ..util import helper\n"),
            [
                "../util.py",
                "../util/__init__.py",
                "../util/helper.py",
                "../util/helper/__init__.py",
            ]
        );
    }

    #[test]
    fn star_import_names_only_the_base() {
        assert_eq!(
            specs("pkg/mod.py", &["pkg"], "from pkg.sub import *\n"),
            ["../pkg/sub.py", "../pkg/sub/__init__.py"]
        );
    }

    #[test]
    fn parenthesized_multiline_names() {
        let src = "from pkg import (\n    a,\n    b as c,\n)\n";
        assert_eq!(
            specs("mod.py", &["pkg"], src),
            [
                "pkg.py",
                "pkg/__init__.py",
                "pkg/a.py",
                "pkg/a/__init__.py",
                "pkg/b.py",
                "pkg/b/__init__.py",
            ]
        );
    }

    #[test]
    fn comments_and_docstrings_are_not_mistaken_for_imports() {
        let src = "\
\"\"\"
import fake.one
\"\"\"
# import fake.two
import real\n";
        assert_eq!(specs("mod.py", &[], src), ["real"]);
    }

    #[test]
    fn raise_from_and_yield_from_are_not_import_statements() {
        let src = "\
def f():
    raise ValueError() from cause
    yield from other()
";
        assert_eq!(specs("mod.py", &[], src), Vec::<String>::new());
    }

    #[test]
    fn indented_and_semicolon_separated_imports_are_still_statements() {
        let src = "\
if True:
    import os
import sys; import re
";
        assert_eq!(specs("mod.py", &[], src), ["os", "sys", "re"]);
    }

    #[test]
    fn only_the_word_import_counts_not_a_longer_identifier() {
        assert_eq!(
            specs("mod.py", &[], "import_module('x')\nreimport = 1\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn is_local_distinguishes_a_produced_path_from_a_module_name() {
        assert!(Python.is_local("../pkg/a.py"));
        assert!(!Python.is_local("numpy"));
        assert!(!Python.is_local("os"));
    }

    #[test]
    fn declared_reads_the_pep_621_project_name() {
        let manifest = "\
[build-system]
requires = [\"hatchling\"]

[project]
name = \"my-package\"
version = \"0.1.0\"
";
        assert_eq!(Python.declared(manifest).as_deref(), Some("my-package"));
        // A commented-out name must not win, matching the import lexer's own discipline.
        let manifest = "\
[project]
# name = \"wrong\"
name = \"right\"
";
        assert_eq!(Python.declared(manifest).as_deref(), Some("right"));
        assert_eq!(Python.declared("[tool.other]\nname = \"x\"\n"), None);
    }

    #[test]
    fn vendored_reads_path_dependencies() {
        let manifest = "\
[tool.uv.sources]
mypackage = { path = \"../mypackage\" }
";
        assert_eq!(Python.vendored(manifest), ["../mypackage"]);
    }

    #[test]
    fn stdlib_covers_the_modules_every_supported_interpreter_ships() {
        for name in ["os", "sys", "typing", "__future__", "tomllib", "dataclasses"] {
            assert!(STDLIB.contains(&name), "{name} should be ambient");
        }
    }
}
