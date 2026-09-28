use clap::Parser;
fn main() {
    if let Err(error) = procface::cli::run(procface::cli::Cli::parse()) {
        if error.kind() != std::io::ErrorKind::BrokenPipe {
            eprintln!("procface: {error}");
            std::process::exit(1);
        }
    }
}
