use std::sync::Arc;

use plc_lang::LanguageRegistry;
use plc_lsp_server::PlcLanguageServer;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Language-aware: the analyzer is chosen per document by file extension
    // (ST gets the full IDE backend; other registered languages get diagnostics).
    let registry = Arc::new(LanguageRegistry::with_builtins());
    let (service, socket) = LspService::new(move |client| {
        PlcLanguageServer::with_registry(client, Arc::clone(&registry))
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
