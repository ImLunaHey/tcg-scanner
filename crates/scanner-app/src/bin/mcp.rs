use clap::Parser;
#[tokio::main]
async fn main() -> std::process::ExitCode {
    match scanner_app::mcp::execute(scanner_app::options::McpOptions::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
