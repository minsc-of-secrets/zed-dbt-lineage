use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

const REPO: &str = "minsc-of-secrets/zed-dbt-lineage";
const LSP_VERSION: &str = env!("CARGO_PKG_VERSION");

struct DbtExtension {
    cached_binary_path: Option<String>,
}

impl DbtExtension {
    fn language_server_binary_path(&mut self, worktree: &zed::Worktree) -> Result<String> {
        if let Some(path) = worktree.which("dbt-lsp") {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path {
            if std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false) {
                return Ok(path.clone());
            }
        }

        let (os, arch) = zed::current_platform();
        let os_str = match os {
            zed::Os::Mac => "macos",
            zed::Os::Linux => "linux",
            zed::Os::Windows => return Err("Windows is not yet supported".into()),
        };
        let arch_str = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::X86 => return Err("32-bit x86 is not supported".into()),
        };

        let binary_name = format!("dbt-lsp-{os_str}-{arch_str}");
        let url = format!(
            "https://github.com/{REPO}/releases/download/v{LSP_VERSION}/{binary_name}"
        );

        zed::download_file(&url, "dbt-lsp", zed::DownloadedFileType::Uncompressed)
            .map_err(|e| format!("failed to download dbt-lsp: {e}"))?;
        zed::make_file_executable("dbt-lsp")
            .map_err(|e| format!("failed to make dbt-lsp executable: {e}"))?;

        self.cached_binary_path = Some("dbt-lsp".into());
        Ok("dbt-lsp".into())
    }
}

impl zed::Extension for DbtExtension {
    fn new() -> Self {
        DbtExtension {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = self.language_server_binary_path(worktree)?;
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

        let relative = settings
            .as_ref()
            .and_then(|s| s.get("manifest_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("target/manifest.json");
        let manifest_path = format!("{}/{}", worktree.root_path(), relative);

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
