use clap::Parser;
fn main() {
    if let Err(error) = scanner_app::setup::execute(scanner_app::setup::options::Options::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
