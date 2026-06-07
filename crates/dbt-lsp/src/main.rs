mod hover;
mod lineage;
mod manifest;

use hover::{parse_cursor, render_hover, CursorTarget};
use lineage::{downstream, upstream};
use manifest::ManifestGraph;

use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    graph: Arc<RwLock<Option<ManifestGraph>>>,
    manifest_path: Arc<RwLock<String>>,
    max_depth: Arc<RwLock<usize>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Read settings from initializationOptions
        if let Some(opts) = params.initialization_options {
            if let Some(path) = opts.get("manifest_path").and_then(|v| v.as_str()) {
                *self.manifest_path.write().await = path.to_string();
            }
            if let Some(depth) = opts.get("max_lineage_depth").and_then(|v| v.as_u64()) {
                *self.max_depth.write().await = depth as usize;
            }
        }

        // Load manifest
        self.reload_manifest().await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::NONE,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let graph_lock = self.graph.read().await;
        let graph = match graph_lock.as_ref() {
            Some(g) => g,
            None => {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: "Run `dbt compile` to generate manifest.json".to_string(),
                    }),
                    range: None,
                }));
            }
        };

        let pos = params.text_document_position_params.position;

        // Read the document text from the URI (file path)
        let uri = params.text_document_position_params.text_document.uri;
        let file_path = uri.to_file_path().ok().and_then(|p| {
            std::fs::read_to_string(&p).ok()
        });

        let line_text = file_path
            .as_deref()
            .and_then(|text| text.lines().nth(pos.line as usize))
            .unwrap_or("")
            .to_string();

        let target = match parse_cursor(&line_text, pos.character as usize) {
            Some(t) => t,
            None => return Ok(None),
        };

        let max_depth = *self.max_depth.read().await;

        // Clone the fields we need so the borrow on `graph` ends before
        // we pass `graph` to upstream/downstream.
        let resolved: Option<(String, String, String)> = match &target {
            CursorTarget::Ref(name) => match graph.find_model(name) {
                Some(n) => Some((n.unique_id.clone(), n.name.clone(), n.original_file_path.clone())),
                None => {
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "Model `{}` not found — manifest may be stale. Run `dbt compile`.",
                                name
                            ),
                        }),
                        range: None,
                    }));
                }
            },
            CursorTarget::Source(src, table) => match graph.find_source(src, table) {
                Some(n) => Some((n.unique_id.clone(), n.name.clone(), n.original_file_path.clone())),
                None => {
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!(
                                "Source `{}.{}` not found — manifest may be stale.",
                                src, table
                            ),
                        }),
                        range: None,
                    }));
                }
            },
        };

        let (node_id, node_name, node_file_path) = resolved.unwrap();
        let up = upstream(graph, &node_id, max_depth);
        let down = downstream(graph, &node_id, max_depth);
        let md = render_hover(&node_name, &node_file_path, &up, &down);

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }
}

impl Backend {
    async fn reload_manifest(&self) {
        let path = self.manifest_path.read().await.clone();
        match std::fs::read_to_string(&path) {
            Ok(json) => match ManifestGraph::from_json(&json) {
                Ok(graph) => {
                    *self.graph.write().await = Some(graph);
                }
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, format!("Failed to parse {}: {}", path, e))
                        .await;
                }
            },
            Err(_) => {
                // manifest not found; hover will show the "run dbt compile" message
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        graph: Arc::new(RwLock::new(None)),
        manifest_path: Arc::new(RwLock::new("target/manifest.json".to_string())),
        max_depth: Arc::new(RwLock::new(5)),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
