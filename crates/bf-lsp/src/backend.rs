use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::diagnostics;

pub struct Backend {
    client: Client,
    documents: DashMap<Url, String>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            documents: DashMap::new(),
        }
    }

    async fn publish_diagnostics(&self, uri: Url) {
        let content = {
            match self.documents.get(&uri) {
                Some(r) => r.clone(),
                None => return,
            }
        };
        let diags = diagnostics::to_lsp_diagnostics(&content);
        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "bf-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.documents.insert(uri.clone(), params.text_document.text);
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            let uri = params.text_document.uri.clone();
            self.documents.insert(uri.clone(), change.text);
            self.publish_diagnostics(uri).await;
        }
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let content = {
            match self.documents.get(&params.text_document.uri) {
                Some(r) => r.clone(),
                None => return Ok(None),
            }
        };
        let config = bf_formatter::FormatterConfig::default();
        let formatted = bf_formatter::format(&content, &config);
        if formatted == content {
            return Ok(None);
        }
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(u32::MAX, u32::MAX),
            },
            new_text: formatted,
        }]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::LspService;

    #[test]
    fn lsp_service_new_compiles() {
        let _ = LspService::new(|client| Backend::new(client));
    }

    #[test]
    fn formatting_produces_output() {
        let result = bf_formatter::format(">+<-", &bf_formatter::FormatterConfig::default());
        assert!(!result.is_empty());
    }
}
