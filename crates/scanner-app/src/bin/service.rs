use clap::Parser;
fn main() -> std::process::ExitCode {
    match scanner_app::service::execute(scanner_app::options::ServiceOptions::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
