//! `lathe-remote --stdio`: serves the Project interface on stdin and stdout, for an app that started
//! it over ssh. `lathe-remote --version` prints the version the app checks before it uses a copy.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => println!("{}", env!("CARGO_PKG_VERSION")),
        Some("--stdio") => {
            if let Err(error) = lathe_remote::server::serve(std::io::stdin().lock(), std::io::stdout()) {
                eprintln!("lathe-remote: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: lathe-remote --stdio | --version");
            std::process::exit(2);
        }
    }
}
