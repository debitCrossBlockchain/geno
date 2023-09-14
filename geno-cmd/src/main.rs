

fn main() {
    if let Err(err) = geno_cmd::cli::run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    std::process::exit(0);
}
