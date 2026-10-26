use zed_extension_api::{self as zed, Result, settings::LspSettings};

const RENK_LSP: &str = "renk-lsp";
const LOCAL_RENK_LSP_SOURCE: &str = "crates/renk/src/bin/renk-lsp.rs";
const LOCAL_RENK_MANIFEST: &str = "crates/renk/Cargo.toml";

struct RenkExtension;

impl zed::Extension for RenkExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != RENK_LSP {
            return Err(format!("unknown language server: {language_server_id}"));
        }

        let binary = LspSettings::for_worktree(RENK_LSP, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let args = binary
            .as_ref()
            .and_then(|binary| binary.arguments.clone())
            .unwrap_or_default();

        if let Some(path) = binary.and_then(|binary| binary.path) {
            Ok(zed::Command {
                command: path,
                args,
                env: Default::default(),
            })
        } else if worktree.read_text_file(LOCAL_RENK_LSP_SOURCE).is_ok()
            && let Some(path) = worktree.which("cargo")
        {
            Ok(zed::Command {
                command: path,
                args: [
                    "run".to_string(),
                    "--quiet".to_string(),
                    "--manifest-path".to_string(),
                    format!("{}/{}", worktree.root_path(), LOCAL_RENK_MANIFEST),
                    "--bin".to_string(),
                    RENK_LSP.to_string(),
                    "--".to_string(),
                ]
                .into_iter()
                .chain(args)
                .collect(),
                env: Default::default(),
            })
        } else if let Some(path) = worktree.which(RENK_LSP) {
            Ok(zed::Command {
                command: path,
                args,
                env: Default::default(),
            })
        } else {
            Err("renk-lsp not found in PATH; install it with `cargo install --path crates/renk --bin renk-lsp` or configure lsp.renk-lsp.binary.path".to_string())
        }
    }
}

zed::register_extension!(RenkExtension);
