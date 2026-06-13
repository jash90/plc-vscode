use plc_lsp_server::PlcLanguageServer;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(PlcLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
