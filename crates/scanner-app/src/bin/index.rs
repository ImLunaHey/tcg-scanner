use clap::Parser;
fn main() {
    if let Err(error) =
        scanner_app::visual_index::execute(scanner_app::visual_index::Options::parse())
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
