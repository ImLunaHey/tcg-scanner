use clap::Parser;
#[tokio::main]
async fn main() -> std::process::ExitCode {
    match scanner_app::cli::execute(scanner_app::options::ClientOptions::parse()).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("JSON value")
            );
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
