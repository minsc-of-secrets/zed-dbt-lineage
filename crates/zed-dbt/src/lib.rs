use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct DbtExtension;

impl zed::Extension for DbtExtension {
    fn new() -> Self {
        DbtExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Look for dbt-lsp binary in PATH or next to the extension
        let binary = worktree
            .which("dbt-lsp")
            .ok_or("dbt-lsp not found in PATH. Install with: cargo install dbt-lsp")?;

        Ok(zed::Command {
            command: binary,
            args: vec![],
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = LspSettings::for_worktree("dbt-lsp", worktree)
            .ok()
            .and_then(|s| s.settings);

        // Default manifest path
        let manifest_path = settings
            .as_ref()
            .and_then(|s| s.get("manifest_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("target/manifest.json")
            .to_string();

        let max_depth = settings
            .as_ref()
            .and_then(|s| s.get("max_lineage_depth"))
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        Ok(Some(serde_json::json!({
            "manifest_path": manifest_path,
            "max_lineage_depth": max_depth
        })))
    }
}

zed::register_extension!(DbtExtension);
