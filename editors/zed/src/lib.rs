use zed_extension_api::{self as zed, LanguageServerId, Result};

struct Zoning;

impl zed::Extension for Zoning {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let command = worktree.which("zoning").ok_or_else(|| {
            "zoning is not on Zed's PATH; install it separately, then restart Zed".to_owned()
        })?;
        Ok(zed::Command {
            command,
            args: vec!["lsp".to_owned(), "--stdio".to_owned()],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(Zoning);
