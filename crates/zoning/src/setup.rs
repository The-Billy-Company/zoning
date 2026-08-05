//! Idempotent editor integration setup owned by the `zoning` executable.

use std::env;
use std::fs;
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::Result;

const EXTENSION_ID: &str = "the-billy-company.zoning";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Editor {
    Cursor,
    Code,
    Zed,
    Neovim,
    Vim,
}

impl Editor {
    fn name(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Code => "VS Code",
            Self::Zed => "Zed",
            Self::Neovim => "Neovim",
            Self::Vim => "Vim",
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Manifest {
    version: String,
    editors: Vec<Editor>,
}

/// Explicit setup operation.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Report detected and installed integrations.
    Status,
    /// Install every detected integration idempotently.
    Run,
    /// Reinstall missing or drifted owned files.
    Repair,
    /// Remove only integrations recorded in zoning's state manifest.
    Uninstall,
}

/// Run an explicit setup action and return human-readable status lines.
///
/// # Errors
/// Returns an error when editor state cannot be read or changed atomically.
pub fn execute(action: Action) -> Result<Vec<String>> {
    let home = home()?;
    execute_in(action, &home)
}

/// Run automatic first-use setup when mutation is safe.
pub fn auto() {
    if env::var_os("CI").is_some()
        || env::var_os("ZONING_NO_SETUP").is_some()
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
        || !std::io::stderr().is_terminal()
    {
        return;
    }
    let Ok(home) = home() else {
        return;
    };
    if state_path(&home).exists() || detect(&home).is_empty() {
        return;
    }
    if let Ok(lines) = execute_in(Action::Run, &home) {
        for line in lines {
            eprintln!("zoning: {line}");
        }
    }
}

fn execute_in(action: Action, home: &Path) -> Result<Vec<String>> {
    let state = state_path(home);
    match action {
        Action::Status => Ok(status(home)),
        Action::Run | Action::Repair => {
            let editors = detect(home);
            if editors.is_empty() {
                return Ok(vec!["no supported editor detected; nothing changed".to_owned()]);
            }
            let mut lines = Vec::new();
            for editor in &editors {
                install(home, *editor)?;
                lines.push(format!("{} integration ready", editor.name()));
            }
            write_atomic(
                &state,
                &serde_json::to_vec_pretty(&Manifest {
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                    editors,
                })?,
            )?;
            Ok(lines)
        }
        Action::Uninstall => {
            let manifest = read_manifest(&state)?;
            let Some(manifest) = manifest else {
                return Ok(vec!["no zoning-owned editor integration is recorded".to_owned()]);
            };
            for editor in manifest.editors {
                remove(home, editor)?;
            }
            if state.exists() {
                fs::remove_file(&state)
                    .map_err(|error| format!("cannot remove {}: {error}", state.display()))?;
            }
            Ok(vec!["removed zoning-owned editor integrations".to_owned()])
        }
    }
}

fn detect(home: &Path) -> Vec<Editor> {
    [
        (Editor::Cursor, command("cursor") || home.join(".cursor").exists()),
        (Editor::Code, command("code") || home.join(".vscode").exists()),
        (
            Editor::Zed,
            command("zed")
                || home.join(".config/zed").exists()
                || home.join("Library/Application Support/Zed").exists(),
        ),
        (Editor::Neovim, command("nvim") || home.join(".config/nvim").exists()),
        (Editor::Vim, command("vim") || home.join(".vim").exists()),
    ]
    .into_iter()
    .filter_map(|(editor, present)| present.then_some(editor))
    .collect()
}

fn status(home: &Path) -> Vec<String> {
    let installed = read_manifest(&state_path(home)).ok().flatten();
    detect(home)
        .into_iter()
        .map(|editor| {
            let ready =
                installed.as_ref().is_some_and(|manifest| manifest.editors.contains(&editor))
                    && present(home, editor);
            format!(
                "{}: {}",
                editor.name(),
                if ready { "installed" } else { "detected, not installed" }
            )
        })
        .collect()
}

/// Whether `editor` actually carries zoning's integration right now.
///
/// Install for Cursor/Code takes one of two doors — the editor's own CLI, or
/// a manual file drop when no CLI is on `PATH` — and only the second leaves
/// an artifact under `installed_path`. Ask the same door install used, or
/// `status`/`repair` would call a CLI-verified install "not installed".
fn present(home: &Path, editor: Editor) -> bool {
    match editor {
        Editor::Cursor | Editor::Code => {
            let cli = if editor == Editor::Cursor { "cursor" } else { "code" };
            if env::var_os("HOME").is_some_and(|actual| Path::new(&actual) == home) && command(cli)
            {
                Command::new(cli).arg("--list-extensions").output().is_ok_and(|listed| {
                    String::from_utf8_lossy(&listed.stdout)
                        .lines()
                        .any(|extension| extension.eq_ignore_ascii_case(EXTENSION_ID))
                })
            } else {
                installed_path(home, editor).exists()
            }
        }
        Editor::Zed => fs::read_to_string(installed_path(home, editor))
            .is_ok_and(|settings| settings.contains("\"zoning\"")),
        Editor::Neovim | Editor::Vim => installed_path(home, editor).exists(),
    }
}

fn install(home: &Path, editor: Editor) -> Result<()> {
    match editor {
        Editor::Cursor | Editor::Code => install_graphical(home, editor),
        Editor::Zed => install_zed(home),
        Editor::Neovim | Editor::Vim => install_vim(home, editor),
    }
}

fn install_graphical(home: &Path, editor: Editor) -> Result<()> {
    let cli = if editor == Editor::Cursor { "cursor" } else { "code" };
    if env::var_os("HOME").is_some_and(|actual| Path::new(&actual) == home) && command(cli) {
        let vsix = home.join(".local/share/zoning/editor-setup/zoning.vsix");
        write_atomic(&vsix, VSIX)?;
        let install = Command::new(cli)
            .args(["--install-extension", &vsix.to_string_lossy(), "--force"])
            .output()
            .map_err(|error| format!("cannot run {cli}: {error}"))?;
        if !install.status.success() {
            return Err(format!(
                "{cli} could not install zoning: {}",
                String::from_utf8_lossy(&install.stderr).trim()
            )
            .into());
        }
        let listed = Command::new(cli)
            .arg("--list-extensions")
            .output()
            .map_err(|error| format!("cannot verify {cli}: {error}"))?;
        if !String::from_utf8_lossy(&listed.stdout)
            .lines()
            .any(|extension| extension.eq_ignore_ascii_case(EXTENSION_ID))
        {
            return Err(format!("{cli} did not report {EXTENSION_ID} after installation").into());
        }
        return Ok(());
    }
    let root = installed_path(home, editor);
    write_text(&root.join("package.json"), &vscode_manifest())?;
    write_text(&root.join("extension.js"), vscode_client())?;
    write_text(&root.join("language-configuration.json"), LANGUAGE_CONFIGURATION)?;
    write_text(&root.join("syntaxes/zoning.tmLanguage.json"), TEXTMATE_GRAMMAR)?;
    write_text(&root.join("icons/zoning.svg"), ICON)?;
    Ok(())
}

fn install_zed(home: &Path) -> Result<()> {
    let settings = if home.join("Library/Application Support/Zed").exists() {
        home.join("Library/Application Support/Zed/settings.json")
    } else {
        home.join(".config/zed/settings.json")
    };
    let existing = fs::read_to_string(&settings).unwrap_or_else(|_| "{}\n".to_owned());
    if existing.contains("\"zoning\"") {
        return Ok(());
    }
    let updated = add_zed_extension(&existing)?;
    write_text(&settings, &updated)
}

fn add_zed_extension(settings: &str) -> Result<String> {
    if settings.contains("\"zoning\"") {
        return Ok(settings.to_owned());
    }
    if let Some(key) = settings.find("\"auto_install_extensions\"") {
        let after = &settings[key..];
        let open = key + after.find('[').ok_or("Zed auto_install_extensions is not an array")?;
        let close =
            open + settings[open..].find(']').ok_or("Zed auto_install_extensions is not closed")?;
        let comma = if settings[open + 1..close].trim().is_empty() { "" } else { ", " };
        let mut updated = settings.to_owned();
        updated.insert_str(close, &format!("{comma}\"zoning\""));
        return Ok(updated);
    }
    let open = settings.find('{').ok_or("Zed settings are not a JSONC object")?;
    let mut updated = settings.to_owned();
    updated.insert_str(open + 1, "\n  \"auto_install_extensions\": [\"zoning\"],");
    Ok(updated)
}

fn install_vim(home: &Path, editor: Editor) -> Result<()> {
    let root = installed_path(home, editor);
    write_text(&root.join("ftdetect/zoning.vim"), VIM_DETECT)?;
    write_text(&root.join("syntax/zoning.vim"), VIM_SYNTAX)?;
    write_text(&root.join("ftplugin/zoning.vim"), VIM_PLUGIN)?;
    if editor == Editor::Neovim {
        write_text(&root.join("plugin/zoning.lua"), NEOVIM_LSP)?;
    }
    Ok(())
}

fn remove(home: &Path, editor: Editor) -> Result<()> {
    if editor == Editor::Zed {
        let path = installed_path(home, editor);
        let Ok(settings) = fs::read_to_string(&path) else {
            return Ok(());
        };
        let updated = settings
            .replace("\"zoning\", ", "")
            .replace(", \"zoning\"", "")
            .replace("\"zoning\"", "");
        return write_text(&path, &updated);
    }
    // Install shelled out to the editor's own CLI when one owns this HOME;
    // uninstall must reverse through the same door, or the extension the
    // editor thinks it manages outlives zoning's own bookkeeping.
    if matches!(editor, Editor::Cursor | Editor::Code) {
        let cli = if editor == Editor::Cursor { "cursor" } else { "code" };
        if env::var_os("HOME").is_some_and(|actual| Path::new(&actual) == home) && command(cli) {
            Command::new(cli)
                .args(["--uninstall-extension", EXTENSION_ID])
                .output()
                .map_err(|error| format!("cannot run {cli}: {error}"))?;
        }
    }
    let path = installed_path(home, editor);
    if path.exists() {
        fs::remove_dir_all(&path)
            .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
    }
    Ok(())
}

fn installed_path(home: &Path, editor: Editor) -> PathBuf {
    let version = env!("CARGO_PKG_VERSION");
    match editor {
        Editor::Cursor => home.join(format!(".cursor/extensions/{EXTENSION_ID}-{version}")),
        Editor::Code => home.join(format!(".vscode/extensions/{EXTENSION_ID}-{version}")),
        Editor::Zed if home.join("Library/Application Support/Zed").exists() => {
            home.join("Library/Application Support/Zed/settings.json")
        }
        Editor::Zed => home.join(".config/zed/settings.json"),
        Editor::Neovim => home.join(".local/share/nvim/site/pack/zoning/start/zoning"),
        Editor::Vim => home.join(".vim/pack/zoning/start/zoning"),
    }
}

fn state_path(home: &Path) -> PathBuf {
    home.join(".local/share/zoning/editor-setup/manifest.json")
}

fn read_manifest(path: &Path) -> Result<Option<Manifest>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            format!("cannot read zoning setup state {}: {error}", path.display()).into()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display()).into()),
    }
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    write_atomic(path, text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot install {}: {error}", path.display()).into())
}

fn command(name: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|path| env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

fn home() -> Result<PathBuf> {
    env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "HOME is not set".into())
}

fn vscode_manifest() -> String {
    format!(
        r#"{{
  "name": "zoning",
  "displayName": "Zoning",
  "publisher": "the-billy-company",
  "version": "{}",
  "engines": {{"vscode": "^1.85.0"}},
  "main": "./extension.js",
  "activationEvents": ["onLanguage:zoning"],
  "contributes": {{
    "languages": [{{"id": "zoning", "aliases": ["Zoning"], "extensions": [".zone"], "configuration": "./language-configuration.json", "icon": {{"light": "./icons/zoning.svg", "dark": "./icons/zoning.svg"}}}}],
    "grammars": [{{"language": "zoning", "scopeName": "source.zoning", "path": "./syntaxes/zoning.tmLanguage.json"}}]
  }}
}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn vscode_client() -> &'static str {
    "'use strict';\nconst vscode = require('vscode');\nconst cp = require('child_process');\nexports.activate = context => { const child = cp.spawn('zoning', ['lsp', '--stdio']); context.subscriptions.push({dispose: () => child.kill()}); };\nexports.deactivate = () => undefined;\n"
}

const LANGUAGE_CONFIGURATION: &str =
    r#"{"comments":{"lineComment":"//"},"brackets":[["{","}"],["[","]"]]}"#;
const TEXTMATE_GRAMMAR: &str = r#"{"scopeName":"source.zoning","patterns":[{"name":"comment.line.double-slash.zoning","match":"//.*$"},{"name":"keyword.control.zoning","match":"\\b(package|root|language|facade|zones|seal|keep|use|forbid|cycles|limit|reach|variance|because|exclude)\\b"},{"name":"string.quoted.double.zoning","begin":"\"","end":"\""}]}"#;
const ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="currentColor" d="M2 3h12v3H2zm2 4h10v3H4zm2 4h8v3H6z"/></svg>"#;
const VIM_DETECT: &str = "autocmd BufRead,BufNewFile *.zone setfiletype zoning\n";
const VIM_SYNTAX: &str = "syntax keyword zoningKeyword package root language facade zones seal keep use forbid cycles limit reach variance because exclude\nsyntax region zoningString start=/\"/ end=/\"/\nsyntax match zoningComment /\\/\\/.*/\nhighlight default link zoningKeyword Keyword\nhighlight default link zoningString String\nhighlight default link zoningComment Comment\n";
const VIM_PLUGIN: &str =
    "setlocal commentstring=//\\ %s\nsetlocal foldmethod=marker\nsetlocal foldmarker={,}\n";
const NEOVIM_LSP: &str = "vim.api.nvim_create_autocmd('FileType', { pattern = 'zoning', callback = function(args) vim.lsp.start({ name = 'zoning', cmd = {'zoning', 'lsp', '--stdio'}, root_dir = vim.fs.root(args.buf, {'.git', 'contract'}) or vim.uv.cwd() }) end })\n";
const VSIX: &[u8] = include_bytes!("setup/payload/zoning.vsix");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zed_edit_is_surgical_and_idempotent() -> Result<()> {
        let original = "{\n  // mine\n  \"theme\": \"A\"\n}\n";
        let once = add_zed_extension(original)?;
        assert!(once.contains("// mine"));
        assert!(once.contains("\"auto_install_extensions\": [\"zoning\"]"));
        assert_eq!(add_zed_extension(&once)?, once);
        Ok(())
    }

    #[test]
    fn fake_home_install_repair_and_uninstall_are_owned() -> Result<()> {
        let thread_name = std::thread::current().name().unwrap_or("test").replace(':', "_");
        let home = std::env::temp_dir().join(format!(
            "zoning-setup-{}-{}",
            std::process::id(),
            thread_name
        ));
        if home.exists() {
            fs::remove_dir_all(&home)?;
        }
        fs::create_dir_all(&home)?;
        let editors = vec![Editor::Cursor, Editor::Code, Editor::Zed, Editor::Neovim, Editor::Vim];
        for editor in &editors {
            install(&home, *editor)?;
            assert!(installed_path(&home, *editor).exists());
        }
        write_atomic(
            &state_path(&home),
            &serde_json::to_vec(&Manifest {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                editors: editors.clone(),
            })?,
        )?;

        let result = execute_in(Action::Uninstall, &home)?;
        assert_eq!(result, ["removed zoning-owned editor integrations"]);
        assert!(!state_path(&home).exists());
        for editor in editors {
            let path = installed_path(&home, editor);
            if editor == Editor::Zed {
                assert!(!fs::read_to_string(path)?.contains("\"zoning\""));
            } else {
                assert!(!path.exists());
            }
        }
        Ok(fs::remove_dir_all(home)?)
    }
}
