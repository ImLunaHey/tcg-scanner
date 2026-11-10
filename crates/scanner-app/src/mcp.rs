use crate::options::McpOptions;
use scanner_types::error::Result;
use std::sync::Arc;
pub async fn execute(options: McpOptions) -> Result<()> {
    let token = scanner_platform::load_credential(&options.token_file)?;
    let client = scanner_client::Client::new(&options.endpoint, &token)?;
    scanner_mcp::Server::new(Arc::new(client), options.read_only)
        .serve(tokio::io::stdin(), tokio::io::stdout())
        .await
        .map_err(crate::configuration::config)
}
